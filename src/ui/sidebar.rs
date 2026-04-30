use eframe::egui;

use crate::app::AppState;
use crate::message::{ReadReceipt, ReadReceiptRequest};

use super::{AbcomApp, AppView};

impl AbcomApp {
    /// Panneau gauche : pairs, groupes, contrôles réseau
    pub(crate) fn show_sidebar_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("peers_panel")
            .resizable(false)
            .exact_width(220.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);

                let (
                    current_network_id,
                    known_networks,
                    peers_all,
                    selected_conv,
                    unread_counts_all,
                    peer_records,
                ) = {
                    let s = self.state.lock().unwrap();
                    let peers = s.peers.clone();
                    let unread = peers
                        .iter()
                        .map(|p| s.unread_count(&p.username))
                        .collect::<Vec<_>>();
                    (
                        s.current_network_id.clone(),
                        s.known_networks.clone(),
                        peers,
                        s.selected_conversation.clone(),
                        unread,
                        s.peer_records.clone(),
                    )
                };

                if self.selected_network_filter.is_none() {
                    self.selected_network_filter = current_network_id.clone();
                }

                // Sélecteur de réseau
                ui.horizontal(|ui| {
                    ui.label("🌐");
                    let current_label = self
                        .selected_network_filter
                        .as_ref()
                        .and_then(|s| known_networks.iter().find(|n| &n.id == s))
                        .map(|n| n.display_name())
                        .unwrap_or_else(|| self.tr("Tous", "All").to_string());
                    egui::ComboBox::from_id_salt("network_filter")
                        .selected_text(&current_label)
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    self.selected_network_filter.is_none(),
                                    self.tr("🌐 Tous les réseaux", "🌐 All networks"),
                                )
                                .clicked()
                            {
                                self.selected_network_filter = None;
                            }
                            for net in &known_networks {
                                let is_selected =
                                    self.selected_network_filter.as_ref() == Some(&net.id);
                                let is_current = current_network_id.as_ref() == Some(&net.id);
                                let label = if is_current {
                                    format!(
                                        "📡 {} ({})",
                                        net.display_name(),
                                        self.tr("actuel", "current")
                                    )
                                } else {
                                    format!("🔌 {}", net.display_name())
                                };
                                if ui.selectable_label(is_selected, label).clicked() {
                                    self.selected_network_filter = Some(net.id.clone());
                                }
                            }
                        });
                });

                // Filtrer les pairs selon le réseau
                let (peers, unread_counts): (Vec<_>, Vec<_>) =
                    if let Some(ref network_id) = self.selected_network_filter {
                        let seen: Vec<&str> = known_networks
                            .iter()
                            .find(|n| &n.id == network_id)
                            .map(|n| n.seen_peers.iter().map(|s| s.as_str()).collect())
                            .unwrap_or_default();
                        peers_all
                            .iter()
                            .zip(unread_counts_all.iter())
                            .filter(|(p, _)| seen.contains(&p.username.as_str()))
                            .map(|(p, u)| (p.clone(), *u))
                            .unzip()
                    } else {
                        (peers_all.clone(), unread_counts_all.clone())
                    };

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

                        let display_name = peer_records
                            .iter()
                            .find(|r| r.username == peer.username)
                            .and_then(|r| r.alias.clone())
                            .unwrap_or_else(|| peer.username.clone());
                        let font_id = egui::TextStyle::Button.resolve(ui.style());
                        ui.painter().text(
                            rect.left_center() + egui::vec2(24.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            &display_name,
                            font_id,
                            ui.visuals().text_color(),
                        );

                        if unread > 0 {
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
                                self.active_view = AppView::Chat;
                                let my_name = s.my_username.clone();
                                let msgs_to_read: Vec<_> = s
                                    .messages
                                    .iter()
                                    .filter(|m| {
                                        m.from == peer_name
                                            && m.to_user == Some(s.my_username.clone())
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
                    if ui.small_button("+").clicked() {
                        self.show_group_modal = true;
                        self.group_name_input.clear();
                        self.group_members_selected.clear();
                    }
                });
                ui.add_space(4.0);

                let groups = self.state.lock().unwrap().groups.clone();
                if groups.is_empty() {
                    ui.weak(self.tr("Aucun groupe", "No group"));
                } else {
                    for group in &groups {
                        let is_selected = selected_conv
                            .as_ref()
                            .map(|c| c == &format!("#{}", group.name))
                            .unwrap_or(false);
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
                            rect.left_center() + egui::vec2(10.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            &format!("🔗 {}", group.name),
                            font_id,
                            ui.visuals().text_color(),
                        );
                        if resp.clicked() {
                            let group_name = format!("#{}", group.name);
                            self.switch_conversation(Some(group_name));
                            self.active_view = AppView::Chat;
                        }
                        ui.add_space(4.0);
                    }
                }

                // Conversation globale
                let is_global = selected_conv.is_none() && self.active_view == AppView::Chat;
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
                        self.active_view = AppView::Chat;
                    }
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let my_name = self.state.lock().unwrap().my_username.clone();
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}",
                            self.tr("Vous", "You"),
                            my_name
                        ))
                        .small(),
                    );
                    ui.add_space(4.0);
                    let btn = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::SelectableLabel::new(
                            self.active_view == AppView::Networks,
                            self.tr("🌐  Gérer les réseaux", "🌐  Manage networks"),
                        ),
                    );
                    if btn.clicked() {
                        self.active_view = if self.active_view == AppView::Networks {
                            AppView::Chat
                        } else {
                            AppView::Networks
                        };
                    }
                });
            });
    }

    /// Bandeau de typage en bas de page
    pub(crate) fn show_typing_panel(&mut self, ctx: &egui::Context) {
        let typing_list = self.state.lock().unwrap().typing_users_list();
        if !typing_list.is_empty() {
            egui::TopBottomPanel::bottom("typing_panel")
                .exact_height(25.0)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "✍ {} {}",
                            typing_list.join(", ")
                            ,self.tr("en train d'écrire...", "is typing...")
                        ))
                        .weak()
                        .small(),
                    );
                });
        }
    }
}
