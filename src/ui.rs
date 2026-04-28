use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{AppEvent, ChatMessage, SendRequest};

fn emoji_font_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share"))
        .join("abcom/NotoEmoji-Regular.ttf")
}

fn configure_fonts(cc: &eframe::CreationContext<'_>) {
    let path = emoji_font_path();
    if let Ok(bytes) = std::fs::read(&path) {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "noto_emoji".to_owned(),
            egui::FontData::from_owned(bytes).into(),
        );
        for family in fonts.families.values_mut() {
            family.push("noto_emoji".to_owned());
        }
        cc.egui_ctx.set_fonts(fonts);
    }
}

pub fn run(
    state: Arc<Mutex<AppState>>,
    event_rx: mpsc::Receiver<AppEvent>,
    send_tx: mpsc::Sender<SendRequest>,
) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Abcom")
            .with_inner_size([860.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Abcom",
        options,
        Box::new(|cc| {
            configure_fonts(cc);
            Ok(Box::new(AbcomApp::new(state, event_rx, send_tx)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(())
}

struct AbcomApp {
    state: Arc<Mutex<AppState>>,
    event_rx: mpsc::Receiver<AppEvent>,
    send_tx: mpsc::Sender<SendRequest>,
    input: String,
    show_emoji_picker: bool,
    last_notification: Option<String>,
    notification_time: std::time::Instant,
    emoji_textures: Vec<(String, egui::TextureHandle)>,
    emoji_textures_loaded: bool,
    emoji_category: usize,
    emoji_map: std::collections::HashMap<String, usize>,
}

fn load_emoji_textures(ctx: &egui::Context) -> Vec<(String, egui::TextureHandle)> {
    crate::emoji_registry::EMOJI_DATA
        .iter()
        .filter_map(|(ch, bytes)| {
            image::load_from_memory(bytes).ok().map(|img| {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    format!("emoji_{ch}"),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                (ch.to_string(), texture)
            })
        })
        .collect()
}


/// Rend un texte contenant des emojis en les affichant comme images PNG colorées.
fn render_inline(
    ui: &mut egui::Ui,
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    textures: &[(String, egui::TextureHandle)],
) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut acc = String::new();
    let size = egui::vec2(16.0, 16.0);

    while i < chars.len() {
        let mut matched = false;
        // Essayer séquences de 2 chars (drapeaux, ZWJ) puis 1 char
        for len in [2usize, 1] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if let Some(&idx) = emoji_map.get(&s) {
                    if !acc.is_empty() {
                        ui.label(egui::RichText::new(&acc).wrap());
                        acc.clear();
                    }
                    if let Some((_, tex)) = textures.get(idx) {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(size));
                    }
                    i += len;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            let ch = chars[i];
            // Ignorer les variation selectors (FE0F) qui ne s'affichent pas
            if ch != '\u{fe0f}' && ch != '\u{200d}' {
                acc.push(ch);
            }
            i += 1;
        }
    }
    if !acc.is_empty() {
        ui.label(egui::RichText::new(&acc).wrap());
    }
}

impl AbcomApp {
    fn new(
        state: Arc<Mutex<AppState>>,
        event_rx: mpsc::Receiver<AppEvent>,
        send_tx: mpsc::Sender<SendRequest>,
    ) -> Self {
        Self {
            state,
            event_rx,
            send_tx,
            input: String::new(),
            show_emoji_picker: false,
            last_notification: None,
            notification_time: std::time::Instant::now(),
            emoji_textures: Vec::new(),
            emoji_textures_loaded: false,
            emoji_category: 0,
            emoji_map: std::collections::HashMap::new(),
        }
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Chargement paresseux des textures emoji (nécessite le contexte egui)
        if !self.emoji_textures_loaded {
            self.emoji_textures = load_emoji_textures(ctx);
            self.emoji_map = self.emoji_textures
                .iter()
                .enumerate()
                .map(|(i, (ch, _))| (ch.clone(), i))
                .collect();
            self.emoji_textures_loaded = true;
        }

        // Dépiler les événements réseau reçus depuis les tâches tokio
        {
            let mut s = self.state.lock().unwrap();
            while let Ok(evt) = self.event_rx.try_recv() {
                match evt {
                    AppEvent::MessageReceived(msg) => {
                        s.add_message(msg.clone());
                        // Notification
                        self.last_notification = Some(format!("{}: {}", msg.from, msg.content));
                        self.notification_time = std::time::Instant::now();
                    }
                    AppEvent::PeerDiscovered { username, addr } => s.add_peer(username, addr),
                    AppEvent::UserTyping(username) => s.set_user_typing(username),
                    AppEvent::UserStoppedTyping(_username) => {
                        s.clear_typing_if_old();
                    }
                }
            }
            s.clear_typing_if_old();
        }

        // Repeindre toutes les 100 ms pour capter les nouveaux messages
        ctx.request_repaint_after(Duration::from_millis(100));

        // ── Panneau gauche : liste des pairs ──────────────────────────────
        egui::SidePanel::left("peers_panel")
            .resizable(false)
            .exact_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.heading("Pairs LAN");
                ui.separator();

                let (peers, selected_conv) = {
                    let s = self.state.lock().unwrap();
                    (s.peers.clone(), s.selected_conversation.clone())
                };

                if peers.is_empty() {
                    ui.add_space(8.0);
                    ui.weak("En attente de pairs...");
                } else {
                    for peer in peers.iter() {
                        let is_selected = selected_conv.as_ref().map(|c| c == &peer.username).unwrap_or(false);
                        let resp = ui.selectable_label(is_selected, format!("● {}", peer.username));
                        if resp.clicked() {
                            self.state.lock().unwrap().selected_conversation =
                                if is_selected { None } else { Some(peer.username.clone()) };
                        }
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let my_name = self.state.lock().unwrap().my_username.clone();
                    ui.separator();
                    ui.label(egui::RichText::new(format!("Connecté : {}", my_name)).small());
                });
            });

        // ── Panneau typage : indicateurs de frappe ─────────────────────────
        let typing_list = self.state.lock().unwrap().typing_users_list();
        if !typing_list.is_empty() {
            let typing_text = format!("✍ {} en train d'écrire...", typing_list.join(", "));
            egui::TopBottomPanel::bottom("typing_panel")
                .exact_height(25.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(typing_text).weak().small());
                });
        }

        // ── Barre du bas : champ de saisie ────────────────────────────────
        egui::TopBottomPanel::bottom("input_panel")
            .exact_height(78.0)  // Hauteur avec bon padding
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    // ─── Destinataire (à gauche) ───
                    let (target, selected_addr, all_peers) = {
                        let s = self.state.lock().unwrap();
                        let target = s
                            .selected_conversation
                            .as_ref()
                            .map(|u| u.clone())
                            .unwrap_or_else(|| "tous".to_string());
                        (target, s.selected_peer_addr(), s.peers.clone())
                    };

                    ui.label(
                        egui::RichText::new(format!("→ {}", target))
                            .color(egui::Color32::from_rgb(100, 180, 255))
                            .size(12.0),
                    );

                    ui.add_space(6.0);

                    // ─── Zone de saisie avec scrollbar interne (2 lignes visibles) ───
                    let available_w = ui.available_width() - 75.0;  // Espace pour les 2 boutons
                    let resp = egui::ScrollArea::vertical()
                        .max_height(32.0)  // Réduit pour 2 lignes max
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.input)
                                    .desired_width(available_w - 12.0)
                                    .desired_rows(1)
                                    .hint_text("Écrire un message…"),
                            )
                        })
                        .inner;

                    ui.add_space(4.0);

                    // ─── Bouton emoji (plus petit) ───
                    let emoji_btn_response = if !self.emoji_textures.is_empty() {
                        let (_ch, tex) = &self.emoji_textures[0];
                        let img_btn = egui::ImageButton::new(
                            egui::Image::new(tex).fit_to_exact_size(egui::vec2(20.0, 20.0)),
                        )
                        .frame(true)
                        .selected(self.show_emoji_picker);
                        ui.add(img_btn)
                    } else {
                        ui.button("😊")
                    };
                    if emoji_btn_response.clicked() {
                        self.show_emoji_picker = !self.show_emoji_picker;
                    }

                    // ─── Bouton envoyer (plus petit) ───
                    let pressed_enter =
                        resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let clicked_send = ui.button("📤").clicked();

                    if (pressed_enter || clicked_send) && !self.input.trim().is_empty() {
                        let content = self.input.trim().to_string();
                        let now = chrono::Local::now().format("%H:%M").to_string();
                        let (my_name, selected_peer_name) = {
                            let s = self.state.lock().unwrap();
                            let my_username = s.my_username.clone();
                            let peer_name = s.selected_conversation.clone();
                            (my_username, peer_name)
                        };
                        
                        let msg = ChatMessage { 
                            from: my_name, 
                            content, 
                            timestamp: now,
                            to_user: selected_peer_name.clone(),  // Direct if peer selected, broadcast if None
                        };

                        self.state.lock().unwrap().add_message(msg.clone());
                        // Also update conversation view to show sent message
                        if let Some(peer_name) = &selected_peer_name {
                            self.state.lock().unwrap().selected_conversation = Some(peer_name.clone());
                        }
                        self.input.clear();

                        if let Some(addr) = selected_addr {
                            let _ = self.send_tx.try_send(SendRequest { to_addr: addr, message: msg });
                        } else {
                            for peer in &all_peers {
                                let _ = self.send_tx.try_send(SendRequest {
                                    to_addr: peer.addr,
                                    message: msg.clone(),
                                });
                            }
                        }

                        resp.request_focus();
                        self.show_emoji_picker = false;
                    }
                });
            });

        // ── Notification popup ─────────────────────────────────────────────
        if let Some(notif) = &self.last_notification {
            let elapsed = self.notification_time.elapsed().as_secs_f32();
            if elapsed < 3.0 {
                // Affichage haut droite
                egui::Window::new("Notification")
                    .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
                    .resizable(false)
                    .collapsible(false)
                    .title_bar(false)
                    .show(ctx, |ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 200, 100),
                            egui::RichText::new(notif).text_style(egui::TextStyle::Body),
                        );
                    });
            } else {
                self.last_notification = None;
            }
        }

        // ── Popup : Picker d'emojis avec catégories ──────────────────────
        if self.show_emoji_picker {
            egui::Window::new("Emojis")
                .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(0.0, -60.0))
                .resizable(false)
                .collapsible(false)
                .fixed_size([310.0, 340.0])
                .show(ctx, |ui| {
                    // Ligne d'icônes de catégories
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (cat_idx, (cat_icon, _start, _end)) in
                            crate::emoji_registry::EMOJI_CATEGORIES.iter().enumerate()
                        {
                            let selected = self.emoji_category == cat_idx;
                            let btn = egui::Button::new(
                                egui::RichText::new(*cat_icon).size(18.0)
                            )
                            .min_size(egui::vec2(24.0, 24.0))
                            .selected(selected)
                            .frame(selected);
                            if ui.add(btn).clicked() {
                                self.emoji_category = cat_idx;
                            }
                        }
                    });
                    ui.separator();

                    // Grille d'emojis — hauteur fixe
                    let (_, start, end) =
                        crate::emoji_registry::EMOJI_CATEGORIES[self.emoji_category];
                    let slice = &self.emoji_textures[start..end.min(self.emoji_textures.len())];

                    egui::ScrollArea::vertical()
                        .max_height(270.0)
                        .min_scrolled_height(270.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Grid::new("emoji_grid")
                                .spacing([3.0, 3.0])
                                .show(ui, |ui| {
                                    for (idx, (ch, texture)) in slice.iter().enumerate() {
                                        let img = egui::Image::new(texture)
                                            .fit_to_exact_size(egui::vec2(34.0, 34.0));
                                        let btn = egui::ImageButton::new(img).frame(false);
                                        if ui.add(btn).on_hover_text(ch.as_str()).clicked() {
                                            self.input.push_str(ch);
                                            self.show_emoji_picker = false;
                                        }
                                        if (idx + 1) % 8 == 0 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        });
                });
        }


        // ── Zone centrale : messages avec conversations ───────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            let (selected_conv, my_name, conv_messages) = {
                let s = self.state.lock().unwrap();
                let selected = s.selected_conversation.clone();
                let my_username = s.my_username.clone();
                let msgs = s.get_conversation_messages();
                let conv_msgs: Vec<ChatMessage> = msgs.into_iter().cloned().collect();
                (selected, my_username, conv_msgs)
            };

            // ── Header avec tabs des conversations ────────────────────
            ui.horizontal(|ui| {
                ui.label("💬 Conversations:");
                ui.separator();
                
                // Global tab
                let is_global_selected = selected_conv.is_none();
                if ui.selectable_label(is_global_selected, "📢 Global").clicked() {
                    self.state.lock().unwrap().selected_conversation = None;
                }

                // User conversations tabs
                let peers: Vec<String> = self.state.lock().unwrap()
                    .peers.iter().map(|p| p.username.clone()).collect();
                for peer in &peers {
                    let is_selected = selected_conv.as_ref() == Some(peer);
                    let display_name = format!("🙋 {}", peer);
                    if ui.selectable_label(is_selected, &display_name).clicked() {
                        self.state.lock().unwrap().selected_conversation = Some(peer.clone());
                    }
                }
            });

            ui.separator();

            // ── Messages filtrés ────────────────────────────────────────
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if conv_messages.is_empty() {
                        ui.add_space(50.0);
                        ui.label(egui::RichText::new("Aucun message").weak());
                    }
                    
                    for msg in &conv_messages {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("[{}]", msg.timestamp))
                                        .color(egui::Color32::DARK_GRAY)
                                        .small(),
                                );
                                let name_color = if msg.from == my_name {
                                    egui::Color32::from_rgb(80, 200, 120)
                                } else {
                                    egui::Color32::from_rgb(100, 180, 255)
                                };
                                ui.label(
                                    egui::RichText::new(format!("{}:", msg.from))
                                        .color(name_color)
                                        .strong(),
                                );
                            });
                            // Message content with word-wrapping
                            ui.spacing_mut().item_spacing.x = 1.0;
                            render_inline(
                                ui,
                                &msg.content,
                                &self.emoji_map,
                                &self.emoji_textures,
                            );
                        });
                    }
                });
        });
    }
}
