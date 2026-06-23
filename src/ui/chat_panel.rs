use eframe::egui;

use crate::app::AppState;
use crate::transfer::{TransferDecision, TransferDirection, TransferStatus};

use super::AbcomApp;

impl AbcomApp {
    /// Zone centrale : fil de la conversation sélectionnée
    pub(crate) fn show_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (selected_conv, my_name, conv_messages) = {
                let s = self.state.lock().unwrap();
                let selected = s.selected_conversation.clone();
                let my_username = s.my_username.clone();
                let msgs: Vec<_> = s.get_conversation_messages().into_iter().cloned().collect();
                (selected, my_username, msgs)
            };

            // Conversation privée (1-à-1) = pair sélectionné qui n'est pas un groupe `#…`
            let private_peer = selected_conv
                .as_deref()
                .filter(|c| !c.starts_with('#'))
                .map(str::to_string);
            let is_broadcast = selected_conv.is_none();

            let conversation_title = match &private_peer {
                Some(user) => self.state.lock().unwrap().peer_display_name(user),
                None => selected_conv
                    .clone()
                    .unwrap_or_else(|| self.tr("Tous", "All").to_string()),
            };

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.heading(&conversation_title);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button(self.tr("Actions", "Actions"), |ui| {
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
                        if let Some(user) = &private_peer {
                            if ui
                                .button(self.tr("Renommer ce contact", "Rename contact"))
                                .clicked()
                            {
                                self.rename_input = self
                                    .state
                                    .lock()
                                    .unwrap()
                                    .peer_records
                                    .iter()
                                    .find(|r| &r.username == user)
                                    .and_then(|r| r.alias.clone())
                                    .unwrap_or_default();
                                self.rename_target = Some(user.clone());
                                ui.close_menu();
                            }
                        }
                        if !is_broadcast
                            && ui
                                .button(self.tr("🗑 Effacer l'historique", "🗑 Clear history"))
                                .clicked()
                            {
                                self.state.lock().unwrap().clear_conversation_history();
                                ui.close_menu();
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
                                ui.label(&peer.username);
                            }
                            if peers.is_empty() {
                                ui.label(self.tr(
                                    "Aucun participant connecté",
                                    "No connected participant",
                                ));
                            }
                        } else {
                            ui.label(format!(
                                "{} ({})",
                                my_name2,
                                self.tr("vous", "you")
                            ));
                            if let Some(peer) = sel_conv {
                                ui.label(&peer);
                            }
                        }
                    });
                self.show_participants = open;
            }

            // Modale de renommage de contact
            if let Some(target) = self.rename_target.clone() {
                // Libellés calculés avant la closure (évite d'emprunter `self`
                // pendant qu'on édite `self.rename_input`).
                let title = self.tr("Renommer le contact", "Rename contact");
                let lbl_original = self.tr("Nom d'origine", "Original name");
                let hint = self.tr("Alias (vide = retirer)", "Alias (empty = remove)");
                let save_lbl = self.tr("Enregistrer", "Save");
                let clear_lbl = self.tr("Retirer l'alias", "Remove alias");

                let mut open = true;
                let mut do_save = false;
                let mut do_clear = false;
                egui::Window::new(title)
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(format!("{}: {}", lbl_original, target));
                        ui.add_space(6.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.rename_input)
                                .hint_text(hint)
                                .desired_width(240.0),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            do_save = ui.button(save_lbl).clicked();
                            do_clear = ui.button(clear_lbl).clicked();
                        });
                    });

                if do_save {
                    let trimmed = self.rename_input.trim();
                    let alias = (!trimmed.is_empty()).then(|| trimmed.to_string());
                    self.state.lock().unwrap().set_peer_alias(&target, alias);
                    self.rename_target = None;
                } else if do_clear {
                    self.state.lock().unwrap().set_peer_alias(&target, None);
                    self.rename_target = None;
                } else if !open {
                    self.rename_target = None;
                }
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

                                // Accusés de réception / lecture
                                if msg.from == my_name {
                                    let read_count = self.state.lock().unwrap()
                                        .get_read_count(AppState::message_hash(msg));
                                    show_receipt(ui, read_count > 0);
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

                    // Transferts et propositions de fichiers, intégrés au fil
                    self.render_transfer_cards(ui, &selected_conv);
                });
        });
    }

    /// Rend, dans le fil de la conversation, les propositions de réception en
    /// attente puis la progression des transferts liés à cette conversation.
    /// Affichées en vue globale (« Tous ») ou dans la conversation du pair.
    fn render_transfer_cards(&mut self, ui: &mut egui::Ui, selected_conv: &Option<String>) {
        // ── Propositions de réception (Accepter / Refuser) ──────────────────
        let offers: Vec<(String, String, String, u64, usize)> = self
            .pending_offers
            .iter()
            .filter(|o| selected_conv.is_none() || selected_conv.as_deref() == Some(o.from.as_str()))
            .map(|o| {
                (
                    o.transfer_id.clone(),
                    o.from.clone(),
                    o.label.clone(),
                    o.total_bytes,
                    o.item_count,
                )
            })
            .collect();

        let mut accept_id: Option<String> = None;
        let mut refuse_id: Option<String> = None;
        for (transfer_id, from, label, total, count) in &offers {
            ui.add_space(6.0);
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(48, 52, 60))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            from,
                            self.tr("vous envoie un fichier", "is sending you a file")
                        ))
                        .strong(),
                    );
                    let detail = if *count > 1 {
                        format!(
                            "{} ({}, {} {})",
                            label,
                            format_bytes(*total),
                            count,
                            self.tr("éléments", "items")
                        )
                    } else {
                        format!("{} ({})", label, format_bytes(*total))
                    };
                    ui.label(egui::RichText::new(detail).small());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(self.tr("Refuser", "Decline")).clicked() {
                            refuse_id = Some(transfer_id.clone());
                        }
                        if ui.button(self.tr("Accepter", "Accept")).clicked() {
                            accept_id = Some(transfer_id.clone());
                        }
                    });
                });
        }
        if let Some(id) = accept_id {
            // Le choix du dossier est différé d'une frame (conflit AppKit macOS).
            self.pending_accept = Some(id);
        }
        if let Some(id) = refuse_id {
            if let Some(pos) = self.pending_offers.iter().position(|o| o.transfer_id == id) {
                let offer = self.pending_offers.remove(pos);
                let _ = offer.decision_tx.send(TransferDecision { accept: false, dest_dir: None });
            }
        }

        // ── Progression des transferts ──────────────────────────────────────
        let mut transfers: Vec<_> = self
            .transfer_progress
            .values()
            .filter(|t| !self.dismissed_transfers.contains(&t.transfer_id))
            .filter(|t| selected_conv.is_none() || selected_conv.as_deref() == Some(t.peer.as_str()))
            .cloned()
            .collect();
        transfers.sort_by(|a, b| a.transfer_id.cmp(&b.transfer_id));

        let mut dismiss_id: Option<String> = None;
        for t in &transfers {
            ui.add_space(6.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let dir = match t.direction {
                        TransferDirection::Upload => self.tr("Envoi", "Sent"),
                        TransferDirection::Download => self.tr("Réception", "Received"),
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {} ({})",
                            dir,
                            t.label,
                            format_bytes(t.total_bytes)
                        ))
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if close_button(ui) {
                            dismiss_id = Some(t.transfer_id.clone());
                        }
                        let status = match t.status {
                            TransferStatus::Queued => self.tr("En attente", "Queued"),
                            TransferStatus::Running => self.tr("En cours", "Running"),
                            TransferStatus::Completed => self.tr("Terminé", "Done"),
                            TransferStatus::Failed => self.tr("Échec", "Failed"),
                            TransferStatus::Rejected => self.tr("Refusé", "Declined"),
                        };
                        ui.label(egui::RichText::new(status).small());
                    });
                });
                if t.status == TransferStatus::Running && t.total_bytes > 0 {
                    let ratio = (t.bytes_done as f32 / t.total_bytes as f32).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(ratio)
                            .show_percentage()
                            .desired_width(ui.available_width()),
                    );
                    if let Some(path) = &t.current_path {
                        ui.label(
                            egui::RichText::new(path)
                                .small()
                                .color(egui::Color32::from_gray(150)),
                        );
                    }
                }
                if !t.detail.is_empty() {
                    ui.label(
                        egui::RichText::new(&t.detail)
                            .small()
                            .color(egui::Color32::from_gray(160)),
                    );
                }
            });
        }
        if let Some(id) = dismiss_id {
            self.dismissed_transfers.insert(id);
        }
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

/// Formate une taille en octets de façon lisible (o / Ko / Mo / Go).
fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} Go", b / GB)
    } else if b >= MB {
        format!("{:.1} Mo", b / MB)
    } else if b >= KB {
        format!("{:.1} Ko", b / KB)
    } else {
        format!("{} o", bytes)
    }
}

/// Petite croix de fermeture peinte (pas de glyphe, rendu fiable). Renvoie `true` au clic.
fn close_button(ui: &mut egui::Ui) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() {
            egui::Color32::from_rgb(230, 120, 120)
        } else {
            egui::Color32::from_gray(150)
        };
        let stroke = egui::Stroke::new(1.5, color);
        let p = ui.painter();
        let c = rect.center();
        let d = 3.5;
        p.line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], stroke);
        p.line_segment([c + egui::vec2(d, -d), c + egui::vec2(-d, d)], stroke);
    }
    resp.clicked()
}

/// Dessine une ou deux coches selon le statut de lecture du message.
/// `read = false` → une coche grise (envoyé) ; `read = true` → deux coches bleues (lu).
fn show_receipt(ui: &mut egui::Ui, read: bool) {
    let w = if read { 17.0_f32 } else { 9.0_f32 };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 12.0), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let color = if read {
        egui::Color32::from_rgb(80, 180, 255)
    } else {
        egui::Color32::from_gray(160)
    };
    let stroke = egui::Stroke::new(1.5, color);
    let p = ui.painter();

    // Dessine une coche à partir du coin gauche du rect donné
    let draw_tick = |ox: f32| {
        let base = rect.left_top() + egui::vec2(ox, 4.0);
        p.line_segment([base, base + egui::vec2(2.5, 3.0)], stroke);
        p.line_segment([base + egui::vec2(2.5, 3.0), base + egui::vec2(8.0, -1.5)], stroke);
    };

    draw_tick(0.0);
    if read {
        draw_tick(6.0); // seconde coche décalée à droite
    }
}
