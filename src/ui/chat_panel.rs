use eframe::egui;

use crate::app::AppState;

use super::{AbcomApp, AppView};

impl AbcomApp {
    /// Zone centrale : messages ou vue réseaux
    pub(crate) fn show_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.active_view == AppView::Networks {
                self.show_networks_view(ui);
                return;
            }

            let (selected_conv, my_name, conv_messages) = {
                let s = self.state.lock().unwrap();
                let selected = s.selected_conversation.clone();
                let my_username = s.my_username.clone();
                let msgs: Vec<_> = s.get_conversation_messages().into_iter().cloned().collect();
                (selected, my_username, msgs)
            };

            let conversation_title = selected_conv.as_deref().unwrap_or("Tous");
            let is_broadcast = selected_conv.is_none();

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| { ui.heading(conversation_title); });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button("▾ Actions", |ui| {
                        let sound_text = if self.enable_sound_notifications {
                            "🔊 Désactiver tous les sons"
                        } else {
                            "🔇 Activer tous les sons"
                        };
                        if ui.button(sound_text).clicked() {
                            self.enable_sound_notifications = !self.enable_sound_notifications;
                            ui.close_menu();
                        }
                        let this_conv = selected_conv.clone();
                        let is_muted = self.muted_conversations.contains(&this_conv);
                        let mute_text = if is_muted { "🔔 Réactiver les sons de ce salon" } else { "🔕 Muet pour ce salon" };
                        if ui.button(mute_text).clicked() {
                            if is_muted { self.muted_conversations.remove(&this_conv); } else { self.muted_conversations.insert(this_conv); }
                            ui.close_menu();
                        }
                        if ui.button("👥 Voir les participants").clicked() {
                            self.show_participants = true;
                            ui.close_menu();
                        }
                        if !is_broadcast {
                            if ui.button("🗑 Effacer l'historique").clicked() {
                                self.state.lock().unwrap().clear_conversation_history();
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
            ui.separator();

            // Popup participants
            if self.show_participants {
                let (conv_name, my_name2, sel_conv, peers) = {
                    let s = self.state.lock().unwrap();
                    (
                        s.selected_conversation.clone().unwrap_or_else(|| "Tous".to_string()),
                        s.my_username.clone(),
                        s.selected_conversation.clone(),
                        s.peers.clone(),
                    )
                };
                let mut open = self.show_participants;
                egui::Window::new("Participants")
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new(format!("Conversation : {}", conv_name)).strong());
                        ui.separator();
                        if sel_conv.is_none() {
                            for peer in &peers {
                                ui.horizontal(|ui| { ui.label("👤"); ui.label(&peer.username); });
                            }
                            if peers.is_empty() { ui.label("Aucun participant connecté"); }
                        } else {
                            ui.horizontal(|ui| { ui.label("👤"); ui.label(format!("{} (vous)", my_name2)); });
                            if let Some(peer) = sel_conv {
                                ui.horizontal(|ui| { ui.label("👤"); ui.label(&peer); });
                            }
                        }
                    });
                self.show_participants = open;
            }

            // Aire de messages
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
                                ui.label(egui::RichText::new(format!("[{}]", msg.timestamp)).color(egui::Color32::DARK_GRAY).small());
                                let name_color = if msg.from == my_name {
                                    egui::Color32::from_rgb(80, 200, 120)
                                } else {
                                    egui::Color32::from_rgb(100, 180, 255)
                                };
                                ui.label(egui::RichText::new(format!("{}:", msg.from)).color(name_color).strong());

                                // Accusés de lecture (messages envoyés)
                                if msg.from == my_name {
                                    let s = self.state.lock().unwrap();
                                    let read_count = s.get_read_count(AppState::message_hash(msg));
                                    if read_count > 0 {
                                        ui.label(egui::RichText::new("✓✓").color(egui::Color32::BLUE).small());
                                    } else {
                                        ui.label(egui::RichText::new("✓").color(egui::Color32::GRAY).small());
                                    }
                                }
                            });
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                super::emoji_picker::render_inline(ui, &msg.content, &self.emoji_map, &self.emoji_textures, 16.0);
                            });
                        });
                    }
                });
        });
    }

    /// Popup de notification en haut à droite
    pub(crate) fn show_notification(&mut self, ctx: &egui::Context) {
        if let Some(notif) = &self.last_notification {
            if self.notification_time.elapsed().as_secs_f32() < 3.0 {
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
    }
}
