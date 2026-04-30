use eframe::egui;

use crate::app::Peer;
use crate::message::{KnownNetwork, PeerRecord};

use super::AbcomApp;

impl AbcomApp {
    pub(crate) fn show_networks_view(&mut self, ui: &mut egui::Ui) {
        let (known_networks, peer_records_snap, peers_snap, current_network_id) = {
            let s = self.state.lock().unwrap();
            (
                s.known_networks.clone(),
                s.peer_records.clone(),
                s.peers.clone(),
                s.current_network_id.clone(),
            )
        };

        if self.selected_network_view.is_none() {
            self.selected_network_view = current_network_id
                .clone()
                .or_else(|| known_networks.first().map(|n| n.id.clone()));
        }

        ui.horizontal_wrapped(|ui| {
            ui.heading(self.tr("🌐 Gérer les réseaux", "🌐 Manage networks"));
            ui.label(
                egui::RichText::new(self.tr(
                    "alias séparés du nom détecté",
                    "aliases stay separate from the detected name",
                ))
                .small()
                .weak(),
            );
        });
        ui.separator();
        ui.add_space(8.0);

        let compact_layout = ui.available_width() < 940.0;

        if compact_layout {
            ui.vertical(|ui| {
                let list_height = (ui.available_height() * 0.36).clamp(180.0, 300.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), list_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.show_networks_list(ui, &known_networks, current_network_id.as_deref());
                    },
                );

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                egui::ScrollArea::vertical()
                    .id_salt("network_details_compact")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.show_network_details(
                            ui,
                            &known_networks,
                            &peer_records_snap,
                            &peers_snap,
                            current_network_id.as_deref(),
                        );
                    });
            });
        } else {
            ui.horizontal_top(|ui| {
                let list_width = (ui.available_width() * 0.30).clamp(240.0, 320.0);
                let details_width = (ui.available_width() - list_width - 16.0).max(320.0);

                ui.allocate_ui_with_layout(
                    egui::vec2(list_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.show_networks_list(ui, &known_networks, current_network_id.as_deref());
                    },
                );

                ui.separator();

                ui.allocate_ui_with_layout(
                    egui::vec2(details_width, ui.available_height()),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("network_details_wide")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                self.show_network_details(
                                    ui,
                                    &known_networks,
                                    &peer_records_snap,
                                    &peers_snap,
                                    current_network_id.as_deref(),
                                );
                            });
                    },
                );
            });
        }
    }

    fn show_networks_list(
        &mut self,
        ui: &mut egui::Ui,
        known_networks: &[KnownNetwork],
        current_network_id: Option<&str>,
    ) {
        let colors = network_colors(ui);
        ui.label(
            egui::RichText::new(match self.ui_language {
                super::UiLanguage::French => {
                    format!("{} réseau(x) connu(s)", known_networks.len())
                }
                super::UiLanguage::English => {
                    format!("{} known network(s)", known_networks.len())
                }
            })
                .strong()
                .color(colors.text)
                .small(),
        );
        ui.add_space(8.0);

        if known_networks.is_empty() {
            ui.label(
                egui::RichText::new(self.tr("Aucun réseau connu", "No known network"))
                    .color(colors.muted),
            );
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for net in known_networks {
                    let is_current = current_network_id == Some(net.id.as_str());
                    let is_selected = self.selected_network_view.as_ref() == Some(&net.id);
                    let text_color = if is_selected {
                        colors.selected_text
                    } else {
                        colors.text
                    };
                    let muted_color = if is_selected {
                        colors.selected_muted
                    } else {
                        colors.muted
                    };

                    let response = egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(12, 10))
                        .corner_radius(8.0)
                        .fill(if is_selected {
                            colors.selected_bg
                        } else {
                            colors.card
                        })
                        .stroke(if is_selected {
                            egui::Stroke::new(1.0, colors.selected_border)
                        } else {
                            egui::Stroke::new(1.0, colors.border)
                        })
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal_top(|ui| {
                                ui.label(
                                    egui::RichText::new(if is_current { "📡" } else { "🔌" })
                                        .color(text_color),
                                );
                                ui.vertical(|ui| {
                                    if let Some(alias) = clean_alias(net.alias.as_deref()) {
                                        ui.label(
                                            egui::RichText::new(alias).strong().color(text_color),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{}: {}",
                                                self.tr("Nom détecté", "Detected name"),
                                                network_base_name(net, self.tr("inconnu", "unknown"))
                                            ))
                                            .small()
                                            .color(muted_color),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new(network_base_name(
                                                net,
                                                self.tr("inconnu", "unknown"),
                                            ))
                                                .strong()
                                                .color(text_color),
                                        );
                                        ui.label(
                                            egui::RichText::new(
                                                self.tr("Aucun alias", "No alias")
                                            )
                                                .small()
                                                .color(muted_color),
                                        );
                                    }

                                    let suffix = if is_current {
                                        format!(" · {}", self.tr("actuel", "current"))
                                    } else {
                                        String::new()
                                    };
                                    ui.label(
                                        egui::RichText::new(match self.ui_language {
                                            super::UiLanguage::French => format!(
                                                "{} pair(s) vu(s){}",
                                                net.seen_peers.len(),
                                                suffix
                                            ),
                                            super::UiLanguage::English => format!(
                                                "{} peer(s) seen{}",
                                                net.seen_peers.len(),
                                                suffix
                                            ),
                                        })
                                        .small()
                                        .color(muted_color),
                                    );
                                });
                            });
                        })
                        .response
                        .interact(egui::Sense::click());

                    if response.clicked() {
                        self.selected_network_view = Some(net.id.clone());
                    }
                    ui.add_space(8.0);
                }
            });
    }

    fn show_network_details(
        &mut self,
        ui: &mut egui::Ui,
        known_networks: &[KnownNetwork],
        peer_records_snap: &[PeerRecord],
        peers_snap: &[Peer],
        current_network_id: Option<&str>,
    ) {
        let colors = network_colors(ui);
        let Some(selected_id) = self.selected_network_view.clone() else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(
                        self.tr("Sélectionnez un réseau dans la liste", "Select a network from the list")
                    )
                        .color(colors.muted),
                );
            });
            return;
        };

        let Some(net) = known_networks.iter().find(|n| n.id == selected_id) else {
            self.selected_network_view = None;
            return;
        };

        let online_count = net
            .seen_peers
            .iter()
            .filter(|username| {
                peers_snap
                    .iter()
                    .any(|p| p.online && &p.username == *username)
            })
            .count();

        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(16, 14))
            .corner_radius(8.0)
            .fill(colors.card)
            .stroke(egui::Stroke::new(1.0, colors.selected_border))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(network_base_name(
                                net,
                                self.tr("inconnu", "unknown"),
                            ))
                                .heading()
                                .strong()
                                .color(colors.text),
                        );
                        if let Some(alias) = clean_alias(net.alias.as_deref()) {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {}",
                                    self.tr("Alias", "Alias"),
                                    alias
                                ))
                                    .strong()
                                    .color(colors.accent),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {}",
                                    self.tr("Alias", "Alias"),
                                    self.tr("non défini", "not set")
                                ))
                                .color(colors.muted),
                            );
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if current_network_id == Some(net.id.as_str()) {
                            ui.label(
                                egui::RichText::new(self.tr("Réseau actuel", "Current network"))
                                    .strong()
                                    .color(colors.positive),
                            );
                        }
                    });
                });

                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    detail_pill(
                        ui,
                        self.tr("Nom détecté", "Detected name"),
                        &network_base_name(net, self.tr("inconnu", "unknown")),
                    );
                    detail_pill(ui, self.tr("Identifiant", "Identifier"), &net.id);
                    detail_pill(
                        ui,
                        self.tr("Sous-réseau", "Subnet"),
                        &subnet_label(net, self.tr("inconnu", "unknown")),
                    );
                    detail_pill(
                        ui,
                        self.tr("Pairs vus", "Peers seen"),
                        &net.seen_peers.len().to_string(),
                    );
                    detail_pill(ui, self.tr("En ligne", "Online"), &online_count.to_string());
                });
            });

        ui.add_space(14.0);
        ui.label(
            egui::RichText::new(self.tr("Ajouter un alias", "Add an alias"))
                .strong()
                .color(colors.text),
        );
        let alias_hint = self.tr("Ex: Maison, Bureau, Lab...", "Ex: Home, Office, Lab...");
        let save_alias_label = self.tr("Enregistrer l'alias", "Save alias");
        let remove_alias_label = self.tr("Retirer", "Remove");
        ui.horizontal(|ui| {
            let entry = self
                .network_alias_edits
                .entry(net.id.clone())
                .or_insert_with(|| net.alias.clone().unwrap_or_default());
            let response = ui.add(
                egui::TextEdit::singleline(entry)
                    .hint_text(alias_hint)
                    .desired_width(320.0),
            );
            let save_alias = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let save_clicked = ui.button(save_alias_label).clicked();
            let clear_clicked = ui.button(remove_alias_label).clicked();

            if save_alias || save_clicked || clear_clicked {
                let new_alias = if clear_clicked {
                    None
                } else {
                    clean_alias(Some(entry.as_str())).map(str::to_owned)
                };
                let mut s = self.state.lock().unwrap();
                if let Some(n) = s.known_networks.iter_mut().find(|n| n.id == net.id) {
                    n.alias = new_alias.clone();
                }
                s.save_networks();
                *entry = new_alias.unwrap_or_default();
            }
        });
        ui.label(
            egui::RichText::new(self.tr(
                "Le nom détecté reste conservé. L'alias sert juste d'étiquette.",
                "The detected name is kept. The alias is only a label.",
            ))
                .small()
                .color(colors.muted),
        );

        ui.separator();
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(self.tr("Pairs vus sur ce réseau", "Peers seen on this network"))
                .strong()
                .color(colors.text),
        );
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                if net.seen_peers.is_empty() {
                    ui.label(
                        egui::RichText::new(self.tr(
                            "Aucun pair enregistré sur ce réseau",
                            "No peer recorded on this network",
                        ))
                            .color(colors.muted),
                    );
                }

                for username in &net.seen_peers {
                    let online = peers_snap
                        .iter()
                        .any(|p| p.online && &p.username == username);
                    let record = peer_records_snap.iter().find(|r| &r.username == username);
                    self.show_peer_network_row(ui, username, record, online);
                    ui.add_space(6.0);
                }
            });

        ui.separator();
        if ui
            .button(
                egui::RichText::new(self.tr("Oublier ce réseau", "Forget this network"))
                    .color(egui::Color32::from_rgb(220, 60, 60)),
            )
            .clicked()
        {
            let mut s = self.state.lock().unwrap();
            s.forget_network(&selected_id);
            s.save_networks();
            drop(s);
            self.selected_network_view = None;
        }
    }

    fn show_peer_network_row(
        &mut self,
        ui: &mut egui::Ui,
        username: &str,
        record: Option<&PeerRecord>,
        online: bool,
    ) {
        let colors = network_colors(ui);
        let compact_layout = ui.available_width() < 520.0;
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(12, 8))
            .corner_radius(8.0)
            .fill(colors.card)
            .stroke(egui::Stroke::new(1.0, colors.border))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if compact_layout {
                    ui.vertical(|ui| {
                        self.render_peer_identity(ui, username, record, online, colors);
                        ui.add_space(8.0);
                        self.show_peer_controls(ui, username, record);
                    });
                } else {
                    ui.horizontal_top(|ui| {
                        self.render_peer_identity(ui, username, record, online, colors);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                            self.show_peer_controls(ui, username, record);
                        });
                    });
                }
            });
    }

    fn show_peer_controls(
        &mut self,
        ui: &mut egui::Ui,
        username: &str,
        record: Option<&PeerRecord>,
    ) {
        if ui.small_button(self.tr("Oublier", "Forget")).clicked() {
            let mut s = self.state.lock().unwrap();
            s.forget_peer(username);
            s.save_peer_records();
        }

        let peer_alias_hint = self.tr("Alias du pair", "Peer alias");
        let ok_label = self.tr("OK", "OK");

        let entry = self
            .peer_alias_edits
            .entry(username.to_string())
            .or_insert_with(|| record.and_then(|r| r.alias.clone()).unwrap_or_default());
        let desired_width = ui.available_width().clamp(140.0, 220.0);
        let response = ui.add(
            egui::TextEdit::singleline(entry)
                .hint_text(peer_alias_hint)
                .desired_width(desired_width),
        );
        let save_alias = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if save_alias || ui.small_button(ok_label).clicked() {
            let new_alias = clean_alias(Some(entry.as_str())).map(str::to_owned);
            let mut s = self.state.lock().unwrap();
            if let Some(r) = s.peer_records.iter_mut().find(|r| r.username == username) {
                r.alias = new_alias.clone();
            }
            s.save_peer_records();
            *entry = new_alias.unwrap_or_default();
        }
    }
}

impl AbcomApp {
    fn render_peer_identity(
        &self,
        ui: &mut egui::Ui,
        username: &str,
        record: Option<&PeerRecord>,
        online: bool,
        colors: NetworkColors,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.label(if online { "🟢" } else { "🔴" });
            ui.vertical(|ui| {
                if let Some(alias) = record.and_then(|r| clean_alias(r.alias.as_deref())) {
                    ui.label(egui::RichText::new(alias).strong().color(colors.text));
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}",
                            self.tr("Nom d'origine", "Original name"),
                            username
                        ))
                        .small()
                        .color(colors.muted),
                    );
                } else {
                    ui.label(egui::RichText::new(username).strong().color(colors.text));
                }
                ui.label(
                    egui::RichText::new(self.peer_detail(record, online))
                        .small()
                        .color(colors.muted),
                );
            });
        });
    }

    fn peer_detail(&self, record: Option<&PeerRecord>, online: bool) -> String {
        let status = if online {
            self.tr("en ligne", "online")
        } else {
            self.tr("hors ligne", "offline")
        };
        let subnet = record
            .and_then(|r| r.last_subnet.as_deref())
            .filter(|s| !s.is_empty())
            .unwrap_or(self.tr("réseau inconnu", "unknown network"));
        format!(
            "{} - {}: {}",
            status,
            self.tr("dernier réseau", "last network"),
            subnet
        )
    }
}

fn network_base_name(net: &KnownNetwork, unknown_label: &str) -> String {
    if !net.id.is_empty() {
        net.id.clone()
    } else {
        subnet_label(net, unknown_label)
    }
}

fn subnet_label(net: &KnownNetwork, unknown_label: &str) -> String {
    if net.subnet.is_empty() {
        unknown_label.to_string()
    } else {
        format!("{}.x", net.subnet)
    }
}

fn clean_alias(alias: Option<&str>) -> Option<&str> {
    alias.map(str::trim).filter(|alias| !alias.is_empty())
}

fn detail_pill(ui: &mut egui::Ui, label: &str, value: &str) {
    let colors = network_colors(ui);
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(8, 5))
        .corner_radius(6.0)
        .fill(colors.pill)
        .stroke(egui::Stroke::new(1.0, colors.border))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).small().color(colors.muted));
                ui.label(egui::RichText::new(value).small().strong().color(colors.text));
            });
        });
}

#[derive(Clone, Copy)]
struct NetworkColors {
    card: egui::Color32,
    pill: egui::Color32,
    border: egui::Color32,
    selected_bg: egui::Color32,
    selected_border: egui::Color32,
    selected_text: egui::Color32,
    selected_muted: egui::Color32,
    text: egui::Color32,
    muted: egui::Color32,
    accent: egui::Color32,
    positive: egui::Color32,
}

fn network_colors(ui: &egui::Ui) -> NetworkColors {
    if ui.visuals().dark_mode {
        NetworkColors {
            card: egui::Color32::from_rgb(31, 41, 55),
            pill: egui::Color32::from_rgb(17, 24, 39),
            border: egui::Color32::from_rgb(75, 85, 99),
            selected_bg: egui::Color32::from_rgb(37, 99, 235),
            selected_border: egui::Color32::from_rgb(147, 197, 253),
            selected_text: egui::Color32::WHITE,
            selected_muted: egui::Color32::from_rgb(219, 234, 254),
            text: egui::Color32::from_rgb(249, 250, 251),
            muted: egui::Color32::from_rgb(209, 213, 219),
            accent: egui::Color32::from_rgb(125, 211, 252),
            positive: egui::Color32::from_rgb(134, 239, 172),
        }
    } else {
        NetworkColors {
            card: egui::Color32::from_rgb(255, 255, 255),
            pill: egui::Color32::from_rgb(243, 244, 246),
            border: egui::Color32::from_rgb(156, 163, 175),
            selected_bg: egui::Color32::from_rgb(29, 78, 216),
            selected_border: egui::Color32::from_rgb(30, 64, 175),
            selected_text: egui::Color32::WHITE,
            selected_muted: egui::Color32::from_rgb(219, 234, 254),
            text: egui::Color32::from_rgb(17, 24, 39),
            muted: egui::Color32::from_rgb(55, 65, 81),
            accent: egui::Color32::from_rgb(3, 105, 161),
            positive: egui::Color32::from_rgb(21, 128, 61),
        }
    }
}
