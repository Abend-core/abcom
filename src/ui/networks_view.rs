use eframe::egui;

use crate::message::GroupAction;

use super::AbcomApp;

impl AbcomApp {
    pub(crate) fn show_networks_view(&mut self, ui: &mut egui::Ui) {
        let (known_networks, peer_records_snap, peers_snap, current_network_id) = {
            let s = self.state.lock().unwrap();
            (s.known_networks.clone(), s.peer_records.clone(), s.peers.clone(), s.current_network_id.clone())
        };

        ui.heading("🌐 Réseaux connus");
        ui.separator();
        ui.add_space(4.0);

        egui::SidePanel::left("networks_list_panel")
            .resizable(false)
            .exact_width(180.0)
            .show_inside(ui, |ui| {
                for net in &known_networks {
                    let is_current = current_network_id.as_ref() == Some(&net.id);
                    let label = if is_current {
                        format!("📡 {} (actuel)", net.display_name())
                    } else {
                        format!("🔌 {}", net.display_name())
                    };
                    let is_selected = self.selected_network_view.as_ref() == Some(&net.id);
                    if ui.selectable_label(is_selected, label).clicked() {
                        self.selected_network_view = if is_selected { None } else { Some(net.id.clone()) };
                    }
                }
                if known_networks.is_empty() {
                    ui.label("(aucun réseau connu)");
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let Some(ref sel_net_id) = self.selected_network_view else {
                ui.centered_and_justified(|ui| { ui.label("Sélectionnez un réseau"); });
                return;
            };

            let Some(net) = known_networks.iter().find(|n| &n.id == sel_net_id) else {
                return;
            };

            // Édition du nom/alias du réseau
            ui.horizontal(|ui| {
                ui.label("Alias :");
                let alias_key = net.id.clone();
                let entry = self.network_alias_edits.entry(alias_key.clone()).or_insert_with(|| net.display_name());
                if ui.text_edit_singleline(entry).lost_focus() {
                    let new_alias = entry.trim().to_string();
                    if !new_alias.is_empty() {
                        let mut s = self.state.lock().unwrap();
                        if let Some(n) = s.known_networks.iter_mut().find(|n| n.id == *sel_net_id) {
                            n.alias = Some(new_alias);
                        }
                        s.save_networks();
                    }
                }
            });
            ui.separator();
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Pairs vus sur ce réseau:").strong());
            ui.add_space(4.0);

            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                for username in &net.seen_peers {
                    let online = peers_snap.iter().any(|p| p.online && &p.username == username);
                    let record = peer_records_snap.iter().find(|r| &r.username == username);
                    let display = record.and_then(|r| r.alias.clone()).unwrap_or_else(|| username.clone());
                    let dot = if online { "🟢" } else { "🔴" };

                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .rounding(8.0)
                        .fill(ui.visuals().extreme_bg_color)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(dot);
                                ui.label(&display);

                                // Édition d'alias pair
                                let alias_key = username.clone();
                                let entry = self.peer_alias_edits.entry(alias_key.clone()).or_insert_with(|| display.clone());
                                let editor = egui::TextEdit::singleline(entry)
                                    .hint_text("Alias...")
                                    .desired_width(90.0);
                                if ui.add(editor).lost_focus() {
                                    let new_alias = entry.trim().to_string();
                                    let mut s = self.state.lock().unwrap();
                                    if let Some(r) = s.peer_records.iter_mut().find(|r| r.username == *username) {
                                        r.alias = if new_alias.is_empty() { None } else { Some(new_alias) };
                                    }
                                    s.save_peer_records();
                                }

                                if ui.small_button("🚫 Oublier").clicked() {
                                    let mut s = self.state.lock().unwrap();
                                    s.forget_peer(username);
                                    s.save_peer_records();
                                }
                            });
                        });
                    ui.add_space(4.0);
                }
            });

            ui.separator();
            ui.add_space(8.0);
            if ui.button(egui::RichText::new("🗑 Oublier ce réseau").color(egui::Color32::from_rgb(220, 60, 60))).clicked() {
                let mut s = self.state.lock().unwrap();
                s.forget_network(sel_net_id);
                s.save_networks();
                drop(s);
                self.selected_network_view = None;
            }
        });
    }
}
