use eframe::egui;

use crate::message::SendGroupRequest;

use super::AbcomApp;

impl AbcomApp {
    /// Modal de création de groupe
    pub(crate) fn render_group_modal(&mut self, ctx: &egui::Context) {
        if !self.show_group_modal {
            return;
        }

        let peers = self.state.lock().unwrap().peers.clone();
        let all_peers: Vec<String> = peers.iter().map(|p| p.username.clone()).collect();
        let mut is_open = true;

        egui::Window::new(self.tr("Créer un groupe", "Create a group"))
            .fixed_size([400.0, 350.0])
            .resizable(true)
            .collapsible(false)
            .open(&mut is_open)
            .show(ctx, |ui| {
                ui.label(self.tr("Nom du groupe:", "Group name:"));
                ui.text_edit_singleline(&mut self.group_name_input);
                ui.add_space(12.0);
                ui.label(self.tr(
                    "Sélectionner les pairs à inviter:",
                    "Select peers to invite:",
                ));
                ui.add_space(8.0);

                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        if all_peers.is_empty() {
                            ui.label(self.tr("(Aucun pair disponible)", "(No peer available)"));
                        } else {
                            for peer in &all_peers {
                                let mut is_selected = self.group_members_selected.contains(peer);
                                if ui.checkbox(&mut is_selected, peer).changed() {
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
                    let is_valid = !trimmed.is_empty()
                        && trimmed.len() <= 50
                        && trimmed
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '-');

                    if is_valid {
                        ui.label(
                            egui::RichText::new(format!("✓ {}", trimmed.len()))
                                .small()
                                .color(egui::Color32::GREEN),
                        );
                    } else if !trimmed.is_empty() {
                        ui.label(
                            egui::RichText::new(self.tr("✗ Nom invalide", "✗ Invalid name"))
                                .small()
                                .color(egui::Color32::RED),
                        );
                    }

                    if ui
                        .add_enabled(is_valid, egui::Button::new(self.tr("✓ Créer", "✓ Create")))
                        .clicked()
                    {
                        let name = trimmed.to_string();
                        let members: Vec<String> =
                            self.group_members_selected.iter().cloned().collect();

                        if let Some(group) = self.state.lock().unwrap().create_group(name, members)
                        {
                            let create_event = crate::message::GroupEvent {
                                action: crate::message::GroupAction::Create {
                                    group: group.clone(),
                                },
                            };
                            let online_peers = self.state.lock().unwrap().get_online_peers();
                            for addr in online_peers {
                                let _ = self.send_group_tx.try_send(SendGroupRequest {
                                    to_addr: addr,
                                    event: create_event.clone(),
                                });
                            }
                            self.show_group_modal = false;
                            self.group_name_input.clear();
                            self.group_members_selected.clear();
                        }
                    }

                    if ui.button(self.tr("✕ Annuler", "✕ Cancel")).clicked() {
                        self.show_group_modal = false;
                        self.group_name_input.clear();
                        self.group_members_selected.clear();
                    }
                });
            });

        if !is_open {
            self.show_group_modal = false;
        }
    }
}
