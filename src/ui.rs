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
}

impl AbcomApp {
    fn new(
        state: Arc<Mutex<AppState>>,
        event_rx: mpsc::Receiver<AppEvent>,
        send_tx: mpsc::Sender<SendRequest>,
    ) -> Self {
        Self { state, event_rx, send_tx, input: String::new() }
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dépiler les événements réseau reçus depuis les tâches tokio
        {
            let mut s = self.state.lock().unwrap();
            while let Ok(evt) = self.event_rx.try_recv() {
                match evt {
                    AppEvent::MessageReceived(msg) => s.add_message(msg),
                    AppEvent::PeerDiscovered { username, addr } => s.add_peer(username, addr),
                }
            }
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

                    let available_w = ui.available_width() - 90.0;
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.input)
                            .desired_width(available_w)
                            .hint_text("Écrire un message…"),
                    );

                    let pressed_enter =
                        resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let clicked_send = ui.button("Envoyer").clicked();

                    if (pressed_enter || clicked_send) && !self.input.trim().is_empty() {
                        let content = self.input.trim().to_string();
                        let now = chrono::Local::now().format("%H:%M").to_string();
                        let my_name = self.state.lock().unwrap().my_username.clone();
                        let msg = ChatMessage { from: my_name, content, timestamp: now };

                        self.state.lock().unwrap().add_message(msg.clone());
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
                    }
                });
            });

        // ── Zone centrale : messages ──────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            let (messages, my_name) = {
                let s = self.state.lock().unwrap();
                (s.messages.clone(), s.my_username.clone())
            };

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for msg in &messages {
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
