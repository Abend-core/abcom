use eframe::egui;

use crate::app::AppState;
use crate::transfer::{TransferDirection, TransferStatus};

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

            let conversation_title = selected_conv
                .as_deref()
                .unwrap_or(self.tr("Tous", "All"));
            let is_broadcast = selected_conv.is_none();

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.heading(conversation_title);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button(self.tr("▾ Actions", "▾ Actions"), |ui| {
                        let sound_text = if self.enable_sound_notifications {
                            self.tr("🔊 Désactiver tous les sons", "🔊 Disable all sounds")
                        } else {
                            self.tr("🔇 Activer tous les sons", "🔇 Enable all sounds")
                        };
                        if ui.button(sound_text).clicked() {
                            self.enable_sound_notifications = !self.enable_sound_notifications;
                            ui.close_menu();
                        }
                        let this_conv = selected_conv.clone();
                        let is_muted = self.muted_conversations.contains(&this_conv);
                        let mute_text = if is_muted {
                            self.tr(
                                "🔔 Réactiver les sons de ce salon",
                                "🔔 Re-enable sounds for this chat",
                            )
                        } else {
                            self.tr("🔕 Muet pour ce salon", "🔕 Mute this chat")
                        };
                        if ui.button(mute_text).clicked() {
                            if is_muted {
                                self.muted_conversations.remove(&this_conv);
                            } else {
                                self.muted_conversations.insert(this_conv);
                            }
                            ui.close_menu();
                        }
                        if ui
                            .button(self.tr("👥 Voir les participants", "👥 View participants"))
                            .clicked()
                        {
                            self.show_participants = true;
                            ui.close_menu();
                        }
                        if !is_broadcast {
                            if ui
                                .button(self.tr("🗑 Effacer l'historique", "🗑 Clear history"))
                                .clicked()
                            {
                                self.state.lock().unwrap().clear_conversation_history();
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
            ui.separator();

            let mut transfer_progress: Vec<_> = self.transfer_progress.values().cloned().collect();
            transfer_progress.sort_by(|left, right| left.transfer_id.cmp(&right.transfer_id));
            if !transfer_progress.is_empty() {
                ui.add_space(6.0);
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(self.tr("Transferts", "Transfers")).strong(),
                    );
                    ui.add_space(6.0);
                    for transfer in transfer_progress.iter().rev().take(4) {
                        let ratio = if transfer.total_bytes == 0 {
                            match transfer.status {
                                TransferStatus::Completed => 1.0,
                                _ => 0.0,
                            }
                        } else {
                            (transfer.bytes_done as f32 / transfer.total_bytes as f32).clamp(0.0, 1.0)
                        };
                        let direction = match transfer.direction {
                            TransferDirection::Upload => self.tr("Envoi", "Upload"),
                            TransferDirection::Download => self.tr("Réception", "Download"),
                        };
                        let status = match transfer.status {
                            TransferStatus::Queued => self.tr("En attente", "Queued"),
                            TransferStatus::Running => self.tr("En cours", "Running"),
                            TransferStatus::Completed => self.tr("Terminé", "Completed"),
                            TransferStatus::Failed => self.tr("Échec", "Failed"),
                        };

                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} {} -> {}",
                                        direction, transfer.label, transfer.peer
                                    ))
                                    .strong(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(status);
                                    },
                                );
                            });
                            if let Some(path) = &transfer.current_path {
                                ui.label(
                                    egui::RichText::new(path)
                                        .small()
                                        .color(egui::Color32::from_rgb(190, 190, 196)),
                                );
                            }
                            ui.add(
                                egui::ProgressBar::new(ratio)
                                    .show_percentage()
                                    .desired_width(ui.available_width()),
                            );
                            if !transfer.detail.is_empty() {
                                ui.label(
                                    egui::RichText::new(&transfer.detail)
                                        .small()
                                        .color(egui::Color32::from_rgb(160, 160, 168)),
                                );
                            }
                        });
                        ui.add_space(4.0);
                    }
                });
                ui.separator();
            }

            // Popup participants
            if self.show_participants {
                let (conv_name, my_name2, sel_conv, peers) = {
                    let s = self.state.lock().unwrap();
                    (
                        s.selected_conversation
                            .clone()
                                .unwrap_or_else(|| self.tr("Tous", "All").to_string()),
                        s.my_username.clone(),
                        s.selected_conversation.clone(),
                        s.peers.clone(),
                    )
                };
                let mut open = self.show_participants;
                egui::Window::new(self.tr("Participants", "Participants"))
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {}",
                                self.tr("Conversation", "Conversation"),
                                conv_name
                            ))
                            .strong(),
                        );
                        ui.separator();
                        if sel_conv.is_none() {
                            for peer in &peers {
                                ui.horizontal(|ui| {
                                    ui.label("👤");
                                    ui.label(&peer.username);
                                });
                            }
                            if peers.is_empty() {
                                ui.label(self.tr(
                                    "Aucun participant connecté",
                                    "No connected participant",
                                ));
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.label("👤");
                                ui.label(format!(
                                    "{} ({})",
                                    my_name2,
                                    self.tr("vous", "you")
                                ));
                            });
                            if let Some(peer) = sel_conv {
                                ui.horizontal(|ui| {
                                    ui.label("👤");
                                    ui.label(&peer);
                                });
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
                        ui.label(
                            egui::RichText::new(self.tr("Aucun message", "No message")).weak(),
                        );
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

                                // Accusés de lecture (messages envoyés)
                                if msg.from == my_name {
                                    let s = self.state.lock().unwrap();
                                    let read_count = s.get_read_count(AppState::message_hash(msg));
                                    if read_count > 0 {
                                        ui.label(
                                            egui::RichText::new("✓✓")
                                                .color(egui::Color32::BLUE)
                                                .small(),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new("✓")
                                                .color(egui::Color32::GRAY)
                                                .small(),
                                        );
                                    }
                                }
                            });
                            super::markdown::render_message_markdown(
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

    /// Popup de notification en haut à droite
    pub(crate) fn show_notification(&mut self, ctx: &egui::Context) {
        if let Some(notif) = &self.last_notification {
            if self.notification_time.elapsed().as_secs_f32() < 3.0 {
                egui::Window::new(self.tr("Notification", "Notification"))
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
