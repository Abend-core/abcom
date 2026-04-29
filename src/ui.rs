use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{AppEvent, ChatMessage, SendRequest, SendGroupRequest};

fn app_icon_data() -> Option<egui::IconData> {
    let data = include_bytes!("../assets/app_icon.jpg");
    eprintln!("[ui] Tentative de chargement de l'icône JPG ({} bytes)", data.len());
    match image::load_from_memory(data) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            eprintln!("[ui] Icône chargée avec succès : {}x{}", w, h);
            Some(egui::IconData {
                rgba: rgba.to_vec(),
                width: w,
                height: h,
            })
        }
        Err(err) => {
            eprintln!("[ui] Erreur de chargement icône JPG : {}", err);
            // Fallback: créer une simple icône 32x32 rouge
            let mut rgba = vec![0u8; 32 * 32 * 4];
            for i in 0..(32 * 32) {
                rgba[i * 4] = 200;     // R
                rgba[i * 4 + 1] = 50;  // G
                rgba[i * 4 + 2] = 50;  // B
                rgba[i * 4 + 3] = 255; // A
            }
            eprintln!("[ui] Utilisation de l'icône par défaut (carrée rouge)");
            Some(egui::IconData {
                rgba,
                width: 32,
                height: 32,
            })
        }
    }
}

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
    send_group_tx: mpsc::Sender<SendGroupRequest>,
) -> anyhow::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Abcom")
        .with_inner_size([860.0, 600.0]);

    if let Some(icon) = app_icon_data() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Abcom",
        options,
        Box::new(|cc| {
            configure_fonts(cc);
            Ok(Box::new(AbcomApp::new(state.clone(), event_rx, send_tx.clone(), send_group_tx.clone())))
        }),
    )
    .map_err(|e| {
        eprintln!("Erreur lors du lancement de l'interface graphique : {}", e);
        eprintln!("Cela peut être dû à un environnement graphique non supporté (par exemple WSL sans pilote OpenGL approprié).");
        eprintln!("Pour déployer sur Windows, utilisez le script d'installation : scripts/install-windows.ps1");
        anyhow::anyhow!("Échec de l'initialisation de l'interface graphique : {}", e)
    })?;

    Ok(())
}

struct AbcomApp {
    state: Arc<Mutex<AppState>>,
    event_rx: mpsc::Receiver<AppEvent>,
    send_tx: mpsc::Sender<SendRequest>,
    send_group_tx: mpsc::Sender<SendGroupRequest>,
    input: String,
    show_emoji_picker: bool,
    show_participants: bool,
    enable_sound_notifications: bool,
    last_notification: Option<String>,
    notification_time: std::time::Instant,
    has_unread: bool,
    window_focused: bool,
    emoji_textures: Vec<(String, egui::TextureHandle)>,
    emoji_textures_loaded: bool,
    emoji_category: usize,
    emoji_map: std::collections::HashMap<String, usize>,
    last_cleanup_time: std::time::Instant,
    // Gestion des groupes
    show_group_modal: bool,
    group_name_input: String,
    group_members_selected: std::collections::HashSet<String>,
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
                        ui.label(&acc);
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
        ui.label(&acc);
    }
}

#[cfg(windows)]
fn play_notification_sound() {
    std::thread::spawn(|| {
        use rodio::source::Source;
        use std::time::Duration;

        let Ok((_stream, stream_handle)) = rodio::OutputStream::try_default() else { return };
        let Ok(sink) = rodio::Sink::try_new(&stream_handle) else { return };

        let tone1 = rodio::source::SineWave::new(880.0)
            .take_duration(Duration::from_millis(80))
            .amplify(0.25);
        let tone2 = rodio::source::SineWave::new(1100.0)
            .take_duration(Duration::from_millis(80))
            .amplify(0.20);

        sink.append(tone1);
        sink.append(tone2);
        sink.sleep_until_end();
    });
}

#[cfg(not(windows))]
fn play_notification_sound() {
    print!("\x07");
}

impl AbcomApp {
    fn new(
        state: Arc<Mutex<AppState>>,
        event_rx: mpsc::Receiver<AppEvent>,
        send_tx: mpsc::Sender<SendRequest>,
        send_group_tx: mpsc::Sender<SendGroupRequest>,
    ) -> Self {
        Self {
            state,
            event_rx,
            send_tx,
            send_group_tx,
            input: String::new(),
            show_emoji_picker: false,
            show_participants: false,
            enable_sound_notifications: true,
            last_notification: None,
            notification_time: std::time::Instant::now(),
            has_unread: false,
            window_focused: true,
            emoji_textures: Vec::new(),
            emoji_textures_loaded: false,
            emoji_category: 0,
            emoji_map: std::collections::HashMap::new(),
            last_cleanup_time: std::time::Instant::now(),
            show_group_modal: false,
            group_name_input: String::new(),
            group_members_selected: std::collections::HashSet::new(),
        }
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Capturer l'état du focus au début (avant le traitement des événements)
        self.window_focused = ctx.input(|i| i.focused);

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
                        if msg.from != s.my_username {
                            // Notification visuelle dans l'app
                            self.last_notification = Some(format!("{}: {}", msg.from, msg.content));
                            self.notification_time = std::time::Instant::now();
                            // Flash barre des tâches si fenêtre pas au premier plan
                            self.has_unread = true;
                            // Son uniquement si la fenêtre n'est pas au premier plan
                            if self.enable_sound_notifications && !self.window_focused {
                                play_notification_sound();
                            }
                        }
                    }
                    AppEvent::PeerDiscovered { username, addr } => s.add_peer(username, addr),
                    AppEvent::PeerDisconnected { username } => {
                        // Marquer le pair hors ligne (sans supprimer l'historique ni la carte)
                        if let Some(peer) = s.peers.iter_mut().find(|p| p.username == username) {
                            peer.online = false;
                        }
                    }
                    AppEvent::UserTyping(username) => s.set_user_typing(username),
                    AppEvent::UserStoppedTyping(_username) => {
                        s.clear_typing_if_old();
                    }
                    AppEvent::GroupEventReceived(evt) => {
                        // Traiter les événements de synchronisation de groupe
                        use crate::message::GroupAction;
                        
                        match evt.action {
                            GroupAction::Create { group } => {
                                // Ajouter le groupe s'il n'existe pas déjà
                                if !s.groups.iter().any(|g| g.name == group.name) {
                                    s.groups.push(group);
                                    s.save_groups();
                                    eprintln!("[ui] Groupe reçu et ajouté: {}", s.groups.last().map(|g| &g.name).unwrap_or(&"".to_string()));
                                }
                            }
                            GroupAction::AddMember { group_name, username } => {
                                if let Some(group) = s.groups.iter_mut().find(|g| g.name == group_name) {
                                    if !group.members.contains(&username) {
                                        group.members.push(username);
                                        s.save_groups();
                                    }
                                }
                            }
                            GroupAction::RemoveMember { group_name, username } => {
                                if let Some(group) = s.groups.iter_mut().find(|g| g.name == group_name) {
                                    group.members.retain(|m| m != &username);
                                    s.save_groups();
                                }
                            }
                            GroupAction::Rename { group_name, new_name } => {
                                if let Some(group) = s.groups.iter_mut().find(|g| g.name == group_name) {
                                    group.name = new_name;
                                    s.save_groups();
                                }
                            }
                            GroupAction::Delete { group_name } => {
                                s.groups.retain(|g| g.name != group_name);
                                s.save_groups();
                            }
                        }
                    }
                }
            }
            s.clear_typing_if_old();
        }

        // Nettoyer les pairs inactifs toutes les 5 secondes (timeout: 15 secondes)
        if self.last_cleanup_time.elapsed().as_secs() >= 5 {
            self.last_cleanup_time = std::time::Instant::now();
            {
                let mut s = self.state.lock().unwrap();
                let _disconnected = s.cleanup_inactive_peers(15);
                // Les pairs sont marqués offline automatiquement, la UI se mettra à jour
            }
        }

        // Repeindre toutes les 100 ms pour capter les nouveaux messages
        ctx.request_repaint_after(Duration::from_millis(100));

        // Flash de la barre des tâches si message non lu et fenêtre non focalisée
        if self.has_unread {
            let focused = ctx.input(|i| i.focused);
            if !focused {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Informational,
                ));
            } else {
                // La fenêtre est revenue au premier plan : effacer le flag
                self.has_unread = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Reset,
                ));
            }
        }

        // ── Panneau gauche : conversations et salons ──────────────────────
        egui::SidePanel::left("peers_panel")
            .resizable(false)
            .exact_width(220.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);

                let (peers, selected_conv, unread_counts) = {
                    let s = self.state.lock().unwrap();
                    let peers = s.peers.clone();
                    let unread_counts = peers
                        .iter()
                        .map(|peer| s.unread_count(&peer.username))
                        .collect::<Vec<_>>();
                    (peers, s.selected_conversation.clone(), unread_counts)
                };

                // Section: Conversations privées
                ui.heading("👥 Conversations");
                ui.add_space(4.0);
                if peers.is_empty() {
                    ui.weak("En attente de pairs...");
                } else {
                    for (idx, peer) in peers.iter().enumerate() {
                        let is_selected = selected_conv.as_ref().map(|c| c == &peer.username).unwrap_or(false);
                        let unread = unread_counts[idx];

                        let desired = egui::vec2(ui.available_width(), 56.0);
                        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
                        let visuals = ui.style().interact(&resp);
                        let fill = if is_selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            visuals.bg_fill
                        };
                        let stroke = if is_selected {
                            ui.visuals().selection.stroke
                        } else {
                            visuals.bg_stroke
                        };

                        ui.painter().rect_filled(rect, 8.0, fill);
                        ui.painter().rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Outside);

                        // Diode de statut (verte = en ligne, rouge = hors ligne)
                        let dot_radius = 5.0;
                        let dot_center = egui::pos2(rect.left() + 10.0, rect.center().y);
                        let dot_color = if peer.online {
                            egui::Color32::from_rgb(50, 200, 80)
                        } else {
                            egui::Color32::from_rgb(180, 40, 40)
                        };
                        ui.painter().circle_filled(dot_center, dot_radius, dot_color);

                        let text_color = ui.visuals().text_color();
                        let font_id = egui::TextStyle::Button.resolve(ui.style());
                        let text_pos = rect.left_center() + egui::vec2(24.0, 0.0);
                        ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, &peer.username, font_id.clone(), text_color);

                        if unread > 0 {
                            let badge_text = if unread > 99 {
                                "99+".to_string()
                            } else {
                                unread.to_string()
                            };
                            let badge_size = 24.0;
                            let badge_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.right() - badge_size - 12.0, rect.center().y - badge_size / 2.0),
                                egui::vec2(badge_size, badge_size),
                            );

                            ui.painter().rect_filled(badge_rect, badge_size / 2.0, egui::Color32::from_rgb(220, 40, 60));
                            ui.painter().text(
                                badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                badge_text,
                                egui::TextStyle::Body.resolve(ui.style()),
                                egui::Color32::WHITE,
                            );
                        }

                        if resp.clicked() {
                            let mut s = self.state.lock().unwrap();
                            if is_selected {
                                s.selected_conversation = None;
                            } else {
                                s.selected_conversation = Some(peer.username.clone());
                                s.mark_conversation_read(&peer.username);
                            }
                        }

                        ui.add_space(4.0);
                    }
                }

                ui.separator();
                ui.add_space(4.0);
                // Section: Groupes
                ui.horizontal(|ui| {
                    ui.heading("🔗 Groupes");
                    if ui.small_button("+").clicked() {
                        self.show_group_modal = true;
                        self.group_name_input.clear();
                        self.group_members_selected.clear();
                    }
                });
                ui.add_space(4.0);

                // Afficher les groupes
                let groups = self.state.lock().unwrap().groups.clone();
                if groups.is_empty() {
                    ui.weak("Aucun groupe");
                } else {
                    for group in &groups {
                        let is_selected = selected_conv.as_ref().map(|c| c == &format!("#{}", group.name)).unwrap_or(false);
                        let desired = egui::vec2(ui.available_width(), 56.0);
                        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
                        let visuals = ui.style().interact(&resp);
                        let fill = if is_selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            visuals.bg_fill
                        };
                        let stroke = if is_selected {
                            ui.visuals().selection.stroke
                        } else {
                            visuals.bg_stroke
                        };

                        ui.painter().rect_filled(rect, 8.0, fill);
                        ui.painter().rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Outside);

                        // Icône de groupe
                        let text_color = ui.visuals().text_color();
                        let font_id = egui::TextStyle::Button.resolve(ui.style());
                        let text_pos = rect.left_center() + egui::vec2(10.0, 0.0);
                        ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, &format!("🔗 {}", group.name), font_id.clone(), text_color);

                        if resp.clicked() {
                            let mut s = self.state.lock().unwrap();
                            s.selected_conversation = Some(format!("#{}", group.name));
                        }

                        ui.add_space(4.0);
                    }
                }

                // Global conversation
                let is_global_selected = selected_conv.is_none();
                let resp = ui.add_sized(
                    [ui.available_width(), 56.0],
                    egui::SelectableLabel::new(is_global_selected, "📢 Tous"),
                );
                if resp.clicked() {
                    self.state.lock().unwrap().selected_conversation = None;
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let my_name = self.state.lock().unwrap().my_username.clone();
                    ui.separator();
                    ui.label(egui::RichText::new(format!("Vous : {}", my_name)).small());
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
        // Cacher la saisie si la conversation sélectionnée est un pair hors ligne
        let selected_peer_online = {
            let s = self.state.lock().unwrap();
            match &s.selected_conversation {
                None => true, // Global = toujours actif
                Some(username) => s.is_peer_online(username),
            }
        };

        if !selected_peer_online {
            egui::TopBottomPanel::bottom("input_panel")
                .exact_height(40.0)
                .show(ctx, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("🔴 Cet utilisateur est hors ligne")
                                .color(egui::Color32::from_rgb(180, 40, 40))
                                .small(),
                        );
                    });
                });
        } else {
        egui::TopBottomPanel::bottom("input_panel")
            .exact_height(68.0)  // Hauteur légère avec padding
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);

                    // ─── Bouton emoji intégré (avant le champ) ───
                    let emoji_btn_response = if !self.emoji_textures.is_empty() {
                        let (_ch, tex) = &self.emoji_textures[0];
                        let img_btn = egui::ImageButton::new(
                            egui::Image::new(tex).fit_to_exact_size(egui::vec2(18.0, 18.0)),
                        )
                        .frame(false)  // Pas de frame pour style Discord
                        .selected(self.show_emoji_picker);
                        ui.add(img_btn)
                    } else {
                        ui.button("😊")
                    };
                    if emoji_btn_response.clicked() {
                        self.show_emoji_picker = !self.show_emoji_picker;
                    }

                    ui.add_space(4.0);

                    // ─── Zone de saisie (prend toute la place disponible) ───
                    let (selected_addr, all_peers) = {
                        let s = self.state.lock().unwrap();
                        (s.selected_peer_addr(), s.peers.clone())
                    };

                    let available_w = ui.available_width() - 8.0;
                    let resp = egui::ScrollArea::vertical()
                        .max_height(32.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.input)
                                    .desired_width(available_w - 12.0)
                                    .desired_rows(1)
                                    .hint_text("Message")
                                    .frame(false),  // Pas de frame pour style Discord
                            )
                        })
                        .inner;

                    ui.add_space(4.0);

                    let pressed_enter = ui.input(|i| {
                        i.key_pressed(egui::Key::Enter) && !i.modifiers.shift
                    });

                    if pressed_enter && !self.input.trim().is_empty() {
                        if self.input.ends_with('\n') {
                            self.input.pop();
                        }
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
                            to_user: selected_peer_name.clone(),
                        };

                        self.state.lock().unwrap().add_message(msg.clone());
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
        } // fin else selected_peer_online

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


        // ── Modal de création de groupe ──────────────────────────────────
        if self.show_group_modal {
            let peers = self.state.lock().unwrap().peers.clone();
            let all_peers: Vec<String> = peers.iter().map(|p| p.username.clone()).collect();
            
            let mut is_open = true;
            egui::Window::new("Créer un groupe")
                .fixed_size([400.0, 350.0])
                .resizable(true)
                .collapsible(false)
                .open(&mut is_open)
                .show(ctx, |ui| {
                    ui.label("Nom du groupe:");
                    ui.text_edit_singleline(&mut self.group_name_input);
                    ui.add_space(12.0);

                    ui.label("Sélectionner les pairs à inviter:");
                    ui.add_space(8.0);

                    // ScrollArea pour la liste des pairs avec checkboxes
                    egui::ScrollArea::vertical()
                        .max_height(150.0)
                        .show(ui, |ui| {
                            if all_peers.is_empty() {
                                ui.label("(Aucun pair disponible)");
                            } else {
                                for peer in &all_peers {
                                    let mut is_selected = self.group_members_selected.contains(peer);
                                    let response = ui.checkbox(&mut is_selected, peer);
                                    if response.changed() {
                                        if is_selected {
                                            self.group_members_selected.insert(peer.clone());
                                        } else {
                                            self.group_members_selected.remove(peer);
                                        }
                                    }
                                }
                            }
                        });

                    ui.add_space(12.0);

                    ui.horizontal(|ui| {
                        let trimmed = self.group_name_input.trim();
                        let is_valid_name = !trimmed.is_empty() && trimmed.len() <= 50 && 
                            trimmed.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
                        
                        if is_valid_name {
                            ui.label(egui::RichText::new(format!("✓ {}", trimmed.len())).small().color(egui::Color32::GREEN));
                        } else if !trimmed.is_empty() {
                            ui.label(egui::RichText::new("✗ Nom invalide").small().color(egui::Color32::RED));
                        }
                        
                        if ui.add_enabled(is_valid_name, egui::Button::new("✓ Créer")).clicked() {
                            let group_name = trimmed.to_string();
                            let members: Vec<String> = self.group_members_selected.iter().cloned().collect();
                            
                            if let Some(group) = self.state.lock().unwrap().create_group(group_name, members) {
                                // Broadcaster le groupe à tous les pairs en ligne
                                let create_event = crate::message::GroupEvent {
                                    action: crate::message::GroupAction::Create { group: group.clone() },
                                };
                                
                                let online_peers = self.state.lock().unwrap().get_online_peers();
                                for addr in online_peers {
                                    let req = SendGroupRequest { to_addr: addr, event: create_event.clone() };
                                    let _ = self.send_group_tx.try_send(req);
                                }
                                
                                self.show_group_modal = false;
                                self.group_name_input.clear();
                                self.group_members_selected.clear();
                            }
                        }

                        if ui.button("✕ Annuler").clicked() {
                            self.show_group_modal = false;
                            self.group_name_input.clear();
                            self.group_members_selected.clear();
                        }
                    });
                });

            // Si la croix (×) a été cliquée
            if !is_open {
                self.show_group_modal = false;
            }
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

            let conversation_title = selected_conv.as_deref().unwrap_or("Tous");
            let is_broadcast = selected_conv.is_none();

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.heading(conversation_title);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button("▾ Actions", |ui| {
                        // Activer/désactiver notifications sonores
                        let sound_text = if self.enable_sound_notifications {
                            "🔊 Désactiver les notifications sonores"
                        } else {
                            "🔇 Activer les notifications sonores"
                        };
                        if ui.button(sound_text).clicked() {
                            self.enable_sound_notifications = !self.enable_sound_notifications;
                            ui.close_menu();
                        }

                        // Voir les participants
                        if ui.button("👥 Voir les participants").clicked() {
                            self.show_participants = true;
                            ui.close_menu();
                        }

                        // Effacer l'historique : pas disponible sur "Tous"
                        if !is_broadcast {
                            if ui.button("🗑 Effacer l'historique").clicked() {
                                self.state.lock().unwrap().clear_conversation_history();
                                ui.close_menu();
                            }
                        }

                        // Quitter le salon : uniquement pour les groupes (pas privé, pas broadcast)
                        // TODO: différencier groupe vs privé quand les groupes seront implémentés
                    });
                });
            });
            ui.separator();

            // ── Popup participants ─────────────────────────────────────
            if self.show_participants {
                let (conv_name, my_name, selected_conv, peers) = {
                    let s = self.state.lock().unwrap();
                    (
                        s.selected_conversation.clone().unwrap_or_else(|| "Tous".to_string()),
                        s.my_username.clone(),
                        s.selected_conversation.clone(),
                        s.peers.clone(),
                    )
                };
                let is_broadcast = selected_conv.is_none();
                let mut open = self.show_participants;
                egui::Window::new("Participants")
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new(format!("Conversation : {}", conv_name)).strong());
                        ui.separator();
                        if is_broadcast {
                            // Afficher tous les peers connectés
                            for peer in &peers {
                                ui.horizontal(|ui| {
                                    ui.label("👤");
                                    ui.label(&peer.username);
                                });
                            }
                            if peers.is_empty() {
                                ui.label("Aucun participant connecté");
                            }
                        } else {
                            // Conversation privée : moi + l'autre
                            ui.horizontal(|ui| {
                                ui.label("👤");
                                ui.label(format!("{} (vous)", my_name));
                            });
                            if let Some(peer) = selected_conv {
                                ui.horizontal(|ui| {
                                    ui.label("👤");
                                    ui.label(&peer);
                                });
                            }
                        }
                    });
                self.show_participants = open;
            }

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
                            // Message content - render inline with horizontal_wrapped for automatic wrapping
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                render_inline(
                                    ui,
                                    &msg.content,
                                    &self.emoji_map,
                                    &self.emoji_textures,
                                );
                            });
                        });
                    }
                });
        });
    }
}
