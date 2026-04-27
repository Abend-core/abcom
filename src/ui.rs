use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{AppEvent, ChatMessage, SendRequest};

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
        Box::new(|_cc| Ok(Box::new(AbcomApp::new(state, event_rx, send_tx)))),
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
}

const EMOJIS: &[&str] = &[
    // Smileys
    "😀", "😃", "😄", "😁", "😆", "😂", "😊", "😇",
    "🙂", "🙃", "😉", "😍", "🥰", "😘", "😚", "😋",
    "😎", "🤓", "🥸", "😏", "😑", "😐", "🤨", "😒",
    "😔", "😌", "😪", "🤐", "🥱", "😬", "😈", "👿",
    // Hearts & Love
    "❤", "🧡", "💛", "💚", "💙", "💜", "🖤", "🤍",
    "🤎", "💔", "💕", "💞", "💓", "💗", "💖", "💘",
    // Hands & Gestures
    "👍", "👎", "👏", "🙌", "🤝", "🤞", "✌", "🤘",
    "🤟", "💪", "🖐", "✋", "👋", "🤚", "🙏", "👌",
    // Objects & Symbols
    "🎉", "🎊", "🎈", "🎁", "🎀", "🎂", "🍰", "🎯",
    "🔥", "💥", "✨", "⭐", "🌟", "💫", "💢", "💯",
    // Food
    "🍕", "🍔", "🍟", "🌭", "🌮", "🌯", "🥙", "🥗",
    "🍗", "🍖", "🌰", "🍎", "🍊", "🍋", "🍌", "🍉",
    // Nature
    "☀", "⛅", "🌤", "🌈", "☁", "⛈", "🌙", "⭐",
    "✨", "🌺", "🌸", "🌼", "🌻", "🌷", "🌹", "🏵",
    // Animals (simple)
    "😺", "😸", "😹", "😻", "😼", "😽", "😾", "😿",
    "🐱", "🐶", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼",
];


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
        }
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

                let (peers, selected) = {
                    let s = self.state.lock().unwrap();
                    (s.peers.clone(), s.selected_peer)
                };

                if peers.is_empty() {
                    ui.add_space(8.0);
                    ui.weak("En attente de pairs...");
                } else {
                    for (i, peer) in peers.iter().enumerate() {
                        let is_selected = selected == Some(i);
                        let resp = ui.selectable_label(is_selected, format!("● {}", peer.username));
                        if resp.clicked() {
                            self.state.lock().unwrap().selected_peer =
                                if is_selected { None } else { Some(i) };
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
            .exact_height(54.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let (target, selected_addr, all_peers) = {
                        let s = self.state.lock().unwrap();
                        let target = s
                            .selected_peer
                            .and_then(|i| s.peers.get(i))
                            .map(|p| p.username.clone())
                            .unwrap_or_else(|| "tous".to_string());
                        (target, s.selected_peer_addr(), s.peers.clone())
                    };

                    ui.label(
                        egui::RichText::new(format!("→ {}", target))
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );

                    let available_w = ui.available_width() - 145.0;
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.input)
                            .desired_width(available_w)
                            .hint_text("Écrire un message…"),
                    );

                    // Bouton smileys
                    if ui.button("😊").clicked() {
                        self.show_emoji_picker = !self.show_emoji_picker;
                    }

                    let pressed_enter =
                        resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let clicked_send = ui.button("Envoyer").clicked();

                    if (pressed_enter || clicked_send) && !self.input.trim().is_empty() {
                        let content = self.input.trim().to_string();
                        let now = chrono::Local::now().format("%H:%M").to_string();
                        let (my_name, selected_peer_name) = {
                            let s = self.state.lock().unwrap();
                            let my_username = s.my_username.clone();
                            let peer_name = s.selected_peer
                                .and_then(|i| s.peers.get(i))
                                .map(|p| p.username.clone());
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

        // ── Popup : Picker d'emojis ───────────────────────────────────────
        if self.show_emoji_picker {
            egui::Window::new("Emojis")
                .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(0.0, -60.0))
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    egui::Grid::new("emoji_grid")
                        .spacing([5.0, 5.0])
                        .max_col_width(30.0)
                        .show(ui, |ui| {
                            for (idx, &emoji) in EMOJIS.iter().enumerate() {
                                if ui.button(emoji).clicked() {
                                    self.input.push_str(emoji);
                                    self.show_emoji_picker = false;
                                }
                                if (idx + 1) % 8 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                });
        }


        // ── Zone centrale : messages avec conversations ───────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            let (conversations, selected_conv, my_name, conv_messages) = {
                let mut s = self.state.lock().unwrap();
                let convs = s.get_conversations();
                let selected = s.selected_conversation.clone();
                let my_username = s.my_username.clone();
                let msgs = s.get_conversation_messages();
                let conv_msgs: Vec<ChatMessage> = msgs.into_iter().cloned().collect();
                (convs, selected, my_username, conv_msgs)
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
                            ui.label(&msg.content);
                        });
                    }
                });
        });
    }
}
