use eframe::egui;

use crate::app::AppState;
use crate::message::{ReadReceipt, ReadReceiptRequest};

use super::AbcomApp;

impl AbcomApp {
    /// Panneau gauche : pairs et groupes
    pub(crate) fn show_sidebar_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("peers_panel")
            .resizable(false)
            .exact_width(220.0)
            .show(ctx, |ui| {
                // Instantané depuis le cache dérivé : pairs, compteurs
                // non-lus et alias ne sont recalculés qu'au changement de
                // génération, pas à chaque frame.
                let peers = self.sidebar_cache.peers.clone();
                let selected_conv = self.sidebar_cache.selected_conversation.clone();
                let unread_counts = self.sidebar_cache.unread.clone();
                let display_names = self.sidebar_cache.display_names.clone();
                let groups = self.sidebar_cache.groups.clone();
                let group_unread = self.sidebar_cache.group_unread.clone();

                // Disposition bas-vers-haut : la barre de paramètres est
                // peinte en premier et reste fixe en bas du panneau ; la
                // zone défilante prend ensuite tout l'espace restant
                // au-dessus, avec un seul ascenseur pour conversations +
                // groupes.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let my_name = self.sidebar_cache.my_username.clone();
                    let settings_tip = self.tr("Paramètres", "Settings");
                    let you_label = self.tr("Vous", "You");
                    ui.add_space(4.0);
                    // « Vous : <instance> » à gauche, engrenage Paramètres à droite
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}: {}", you_label, my_name)).small(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if gear_button(ui).on_hover_text(settings_tip).clicked() {
                                self.settings_tab = super::SettingsTab::General;
                                self.show_settings = true;
                            }
                        });
                    });
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("sidebar_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                self.show_sidebar_list(
                                    ui,
                                    &peers,
                                    &selected_conv,
                                    &unread_counts,
                                    &display_names,
                                    &groups,
                                    &group_unread,
                                );
                            });
                        });
                });
            });
    }

    /// Contenu défilant du panneau gauche : conversations, groupes, « Tous ».
    #[allow(clippy::too_many_arguments)]
    fn show_sidebar_list(
        &mut self,
        ui: &mut egui::Ui,
        peers: &[crate::app::Peer],
        selected_conv: &Option<String>,
        unread_counts: &[usize],
        display_names: &[String],
        groups: &[crate::message::Group],
        group_unread: &[usize],
    ) {
        ui.add_space(6.0);

        // Section conversations
        ui.heading(self.tr("👥 Conversations", "👥 Conversations"));
        ui.add_space(4.0);
        if peers.is_empty() {
            ui.weak(self.tr("En attente de pairs...", "Waiting for peers..."));
        } else {
            for (idx, peer) in peers.iter().enumerate() {
                let is_selected = selected_conv
                    .as_ref()
                    .map(|c| c == &peer.username)
                    .unwrap_or(false);
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
                ui.painter()
                    .rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Outside);

                let dot_color = if peer.online {
                    egui::Color32::from_rgb(50, 200, 80)
                } else {
                    egui::Color32::from_rgb(180, 40, 40)
                };
                ui.painter().circle_filled(
                    egui::pos2(rect.left() + 10.0, rect.center().y),
                    5.0,
                    dot_color,
                );

                let display_name = display_names
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| peer.username.clone());
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                ui.painter().text(
                    rect.left_center() + egui::vec2(24.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &display_name,
                    font_id,
                    ui.visuals().text_color(),
                );

                paint_unread_badge(ui, rect, unread);

                if resp.clicked() {
                    let (is_selected_now, peer_name, peer_addr_for_receipt) = {
                        let s = self.state.lock().unwrap();
                        let is_sel = s
                            .selected_conversation
                            .as_ref()
                            .map(|c| c == &peer.username)
                            .unwrap_or(false);
                        let peer_name = peer.username.clone();
                        let peer_addr = peer.addr;
                        (is_sel, peer_name, peer_addr)
                    };

                    if is_selected_now {
                        self.switch_conversation(None);
                    } else {
                        self.switch_conversation(Some(peer_name.clone()));
                        let mut s = self.state.lock().unwrap();
                        s.mark_conversation_read(&peer_name);
                        let my_name = s.my_username.clone();
                        let msgs_to_read: Vec<_> = s
                            .messages
                            .iter()
                            .filter(|m| {
                                m.from == peer_name && m.to_user == Some(s.my_username.clone())
                            })
                            .cloned()
                            .collect();
                        drop(s);
                        for msg in msgs_to_read {
                            let msg_hash = AppState::message_hash(&msg);
                            let receipt = ReadReceipt {
                                from: my_name.clone(),
                                to: msg.from.clone(),
                                message_hash: msg_hash,
                                timestamp: chrono::Local::now().format("%H:%M").to_string(),
                            };
                            let req = ReadReceiptRequest {
                                to_addr: peer_addr_for_receipt,
                                receipt,
                            };
                            let _ = self.send_read_receipt_tx.try_send(req);
                        }
                    }
                }
                ui.add_space(4.0);
            }
        }

        ui.separator();
        ui.add_space(4.0);

        // Section groupes
        ui.horizontal(|ui| {
            ui.heading(self.tr("🔗 Groupes", "🔗 Groups"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("＋")
                    .on_hover_text(self.tr("Créer un groupe", "Create a group"))
                    .clicked()
                {
                    self.show_group_modal = true;
                    self.group_name_input.clear();
                    self.group_members_selected.clear();
                }
            });
        });
        ui.add_space(4.0);

        if groups.is_empty() {
            ui.weak(self.tr("Aucun groupe", "No group"));
        } else {
            for (gidx, group) in groups.iter().enumerate() {
                let conv_key = format!("#{}", group.name);
                let is_selected = selected_conv.as_deref() == Some(conv_key.as_str());
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
                ui.painter()
                    .rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Outside);
                let font_id = egui::TextStyle::Button.resolve(ui.style());
                ui.painter().text(
                    rect.left_center() + egui::vec2(10.0, -9.0),
                    egui::Align2::LEFT_CENTER,
                    format!("🔗 {}", group.name),
                    font_id,
                    ui.visuals().text_color(),
                );
                let n = group.members.len();
                let members_label = if n > 1 {
                    format!("{} {}", n, self.tr("membres", "members"))
                } else {
                    format!("{} {}", n, self.tr("membre", "member"))
                };
                ui.painter().text(
                    rect.left_center() + egui::vec2(10.0, 9.0),
                    egui::Align2::LEFT_CENTER,
                    members_label,
                    egui::TextStyle::Small.resolve(ui.style()),
                    ui.visuals().weak_text_color(),
                );
                paint_unread_badge(ui, rect, group_unread.get(gidx).copied().unwrap_or(0));
                if resp.clicked() {
                    if is_selected {
                        self.switch_conversation(None);
                    } else {
                        self.switch_conversation(Some(conv_key.clone()));
                        self.state.lock().unwrap().mark_conversation_read(&conv_key);
                    }
                }
                ui.add_space(4.0);
            }
        }

        // Conversation globale
        let is_global = selected_conv.is_none();
        {
            let desired = egui::vec2(ui.available_width(), 56.0);
            let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
            let visuals = ui.style().interact(&resp);
            let fill = if is_global {
                ui.visuals().selection.bg_fill
            } else {
                visuals.bg_fill
            };
            let stroke = if is_global {
                ui.visuals().selection.stroke
            } else {
                visuals.bg_stroke
            };
            ui.painter().rect_filled(rect, 8.0, fill);
            ui.painter()
                .rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Outside);
            let font_id = egui::TextStyle::Button.resolve(ui.style());
            ui.painter().text(
                rect.left_center() + egui::vec2(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                self.tr("📢 Tous", "📢 All"),
                font_id,
                ui.visuals().text_color(),
            );
            if resp.clicked() {
                self.switch_conversation(None);
            }
        }
    }
}

/// Pastille rouge de messages non-lus, alignée à droite d'une ligne de la
/// barre latérale (pairs et salons).
fn paint_unread_badge(ui: &egui::Ui, rect: egui::Rect, unread: usize) {
    if unread == 0 {
        return;
    }
    let badge_text = if unread > 99 {
        "99+".to_string()
    } else {
        unread.to_string()
    };
    let badge_size = 24.0;
    let badge_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - badge_size - 12.0,
            rect.center().y - badge_size / 2.0,
        ),
        egui::vec2(badge_size, badge_size),
    );
    ui.painter().rect_filled(
        badge_rect,
        badge_size / 2.0,
        egui::Color32::from_rgb(220, 40, 60),
    );
    ui.painter().text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        badge_text,
        egui::TextStyle::Body.resolve(ui.style()),
        egui::Color32::WHITE,
    );
}

/// Bouton « engrenage » peint (rendu fiable, sans dépendre d'un glyphe emoji).
fn gear_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() {
            ui.visuals().widgets.hovered.fg_stroke.color
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };
        let stroke = egui::Stroke::new(1.4, color);
        let painter = ui.painter();
        let c = rect.center();
        let (r_ring, r_teeth, r_hole) = (4.8, 7.5, 2.0);
        for k in 0..8 {
            let angle = k as f32 * std::f32::consts::TAU / 8.0;
            let dir = egui::vec2(angle.cos(), angle.sin());
            painter.line_segment([c + dir * r_ring, c + dir * r_teeth], stroke);
        }
        painter.circle_stroke(c, r_ring, stroke);
        painter.circle_stroke(c, r_hole, stroke);
    }
    resp
}
