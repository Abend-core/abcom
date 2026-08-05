use eframe::egui;

use crate::message::{GroupAction, GroupEvent, SendGroupRequest};
use crate::util::MutexExt;

use super::AbcomApp;

/// Action destructrice du modal de gestion en attente de confirmation
/// (bouton en deux temps : « Quitter » puis « Confirmer »).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroupConfirmAction {
    Leave,
    Delete,
}

/// Petite croix peinte pour exclure un membre (glyphe « ✕ » non rendu de
/// façon fiable par la police, voir `input_bar::chip_remove_button`).
fn kick_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() {
            ui.visuals().widgets.hovered.fg_stroke.color
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };
        let stroke = egui::Stroke::new(1.6, color);
        let c = rect.center();
        let d = 5.0;
        let p = ui.painter();
        p.line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], stroke);
        p.line_segment([c + egui::vec2(d, -d), c + egui::vec2(-d, d)], stroke);
    }
    resp
}

/// Pastille de présence (vert = en ligne, rouge = hors ligne).
fn presence_dot(ui: &mut egui::Ui, online: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    let color = if online {
        egui::Color32::from_rgb(50, 200, 80)
    } else {
        egui::Color32::from_rgb(180, 40, 40)
    };
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

impl AbcomApp {
    /// Diffuse un événement de groupe aux adresses données.
    fn send_group_event(&self, addrs: &[std::net::SocketAddr], action: GroupAction) {
        let event = GroupEvent { action };
        for addr in addrs {
            let _ = self.net.send_group_tx.try_send(SendGroupRequest {
                to_addr: *addr,
                event: event.clone(),
            });
        }
    }

    /// Modal de création de groupe : nom validé en direct (charte + doublon),
    /// sélection des membres avec présence, créateur inclus d'office.
    pub(crate) fn render_group_modal(&mut self, ctx: &egui::Context) {
        if !self.show_group_modal {
            return;
        }

        let peers = self.sidebar_cache.peers.clone();
        let display_names = self.sidebar_cache.display_names.clone();
        let groups = self.sidebar_cache.groups.clone();

        // Validation vivante, avant la fenêtre : le bouton « Créer » n'est
        // actif que si tout est bon (le create_group ne peut alors échouer
        // que sur une course réseau, signalée en notification).
        let trimmed = self.group_name_input.trim().to_string();
        let name_valid = !trimmed.is_empty()
            && trimmed.len() <= 50
            && trimmed
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        let duplicate = groups.iter().any(|g| g.name.eq_ignore_ascii_case(&trimmed));
        let can_create = name_valid && !duplicate;

        // Libellés résolus avant la closure.
        let title = self.tr("Créer un groupe", "Create a group");
        let lbl_name = self.tr("Nom du groupe", "Group name");
        let hint_name = self.tr("ex : equipe-projet", "e.g. project-team");
        let err_invalid = self.tr(
            "Lettres, chiffres, - et _ uniquement (50 max)",
            "Letters, digits, - and _ only (50 max)",
        );
        let err_duplicate = self.tr(
            "Ce nom de groupe existe déjà",
            "This group name already exists",
        );
        let lbl_members = self.tr("Membres", "Members");
        let note_self = self.tr(
            "Vous êtes inclus automatiquement.",
            "You are included automatically.",
        );
        let no_peer = self.tr(
            "Aucun pair détecté sur le réseau",
            "No peer detected on the network",
        );
        let sel_one = self.tr("membre sélectionné", "member selected");
        let sel_many = self.tr("membres sélectionnés", "members selected");
        let btn_create = self.tr("Créer le groupe", "Create group");
        let btn_cancel = self.tr("Annuler", "Cancel");

        let mut is_open = true;
        let mut do_create = false;
        let mut do_cancel = false;

        egui::Window::new(title)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .resizable(false)
            .collapsible(false)
            .default_width(360.0)
            .open(&mut is_open)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;

                ui.label(egui::RichText::new(lbl_name).strong());
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.group_name_input)
                            .hint_text(hint_name)
                            .desired_width(250.0),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}/50", self.group_name_input.trim().len()))
                            .small()
                            .weak(),
                    );
                    // Entrée dans le champ = créer, si tout est valide.
                    if resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && can_create
                    {
                        do_create = true;
                    }
                });
                if !trimmed.is_empty() {
                    if !name_valid {
                        ui.label(
                            egui::RichText::new(err_invalid)
                                .small()
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                    } else if duplicate {
                        ui.label(
                            egui::RichText::new(err_duplicate)
                                .small()
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                    }
                }

                ui.add_space(2.0);
                ui.separator();
                ui.label(egui::RichText::new(lbl_members).strong());
                ui.label(egui::RichText::new(note_self).small().weak());

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_width(320.0);
                    egui::ScrollArea::vertical()
                        .max_height(170.0)
                        .show(ui, |ui| {
                            if peers.is_empty() {
                                ui.weak(no_peer);
                            } else {
                                for (idx, peer) in peers.iter().enumerate() {
                                    let mut selected =
                                        self.group_members_selected.contains(&peer.username);
                                    ui.horizontal(|ui| {
                                        presence_dot(ui, peer.online);
                                        let label = display_names
                                            .get(idx)
                                            .cloned()
                                            .unwrap_or_else(|| peer.username.clone());
                                        if ui.checkbox(&mut selected, label).changed() {
                                            if selected {
                                                self.group_members_selected
                                                    .insert(peer.username.clone());
                                            } else {
                                                self.group_members_selected.remove(&peer.username);
                                            }
                                        }
                                    });
                                }
                            }
                        });
                });
                let count = self.group_members_selected.len();
                ui.label(
                    egui::RichText::new(format!(
                        "{} {}",
                        count,
                        if count > 1 { sel_many } else { sel_one }
                    ))
                    .small()
                    .weak(),
                );

                ui.add_space(4.0);
                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            can_create,
                            egui::Button::new(egui::RichText::new(btn_create).strong()),
                        )
                        .clicked()
                    {
                        do_create = true;
                    }
                    if ui.button(btn_cancel).clicked() {
                        do_cancel = true;
                    }
                });
            });

        if do_create {
            let members: Vec<String> = self.group_members_selected.iter().cloned().collect();
            // Un seul passage sous verrou : création + adresses des membres,
            // envoi réseau hors verrou. (L'ancien code re-verrouillait dans le
            // corps d'un `if let` dont le garde du scrutinee vivait encore :
            // deadlock — gel de l'application — à chaque création.)
            let created = {
                let mut s = self.state.lock_safe();
                s.create_group(trimmed.clone(), members)
                    .map(|g| (g.clone(), s.group_member_addrs(&g.name)))
            };
            match created {
                Some((group, addrs)) => {
                    let conv = format!("#{}", group.name);
                    self.send_group_event(&addrs, GroupAction::Create { group });
                    self.show_group_modal = false;
                    self.group_name_input.clear();
                    self.group_members_selected.clear();
                    // Ouvre directement le salon créé.
                    self.switch_conversation(Some(conv));
                }
                None => {
                    self.last_notification = Some(
                        self.tr(
                            "Création du groupe impossible",
                            "Could not create the group",
                        )
                        .to_string(),
                    );
                    self.notification_time = std::time::Instant::now();
                }
            }
        }

        if do_cancel || !is_open {
            self.show_group_modal = false;
            self.group_name_input.clear();
            self.group_members_selected.clear();
        }
    }

    /// Modal de gestion d'un salon : liste des membres (présence, couronne du
    /// propriétaire), ajout et exclusion (propriétaire), départ (tous),
    /// suppression (propriétaire) — actions destructrices confirmées en
    /// deux temps.
    pub(crate) fn render_group_manage_modal(&mut self, ctx: &egui::Context) {
        let Some(group_name) = self.group_manage_target.clone() else {
            return;
        };
        // Groupe disparu entre-temps (suppression ou exclusion reçue du
        // réseau) : fermer le modal sans rien afficher.
        let Some(group) = self
            .sidebar_cache
            .groups
            .iter()
            .find(|g| g.name == group_name)
            .cloned()
        else {
            self.group_manage_target = None;
            self.group_manage_confirm = None;
            return;
        };

        let my_name = self.sidebar_cache.my_username.clone();
        let am_owner = group.owner == my_name;
        let peers = self.sidebar_cache.peers.clone();
        let display_names = self.sidebar_cache.display_names.clone();
        let display_of = |name: &str| -> String {
            peers
                .iter()
                .position(|p| p.username == name)
                .and_then(|i| display_names.get(i).cloned())
                .unwrap_or_else(|| name.to_string())
        };
        let online_of =
            |name: &str| name == my_name || peers.iter().any(|p| p.username == name && p.online);

        // Libellés.
        let lbl_owner = self.tr("propriétaire", "owner");
        let lbl_created = self.tr("créé le", "created");
        let lbl_members = self.tr("Membres", "Members");
        let lbl_you = self.tr("(vous)", "(you)");
        let lbl_kick = self.tr("Exclure ce membre", "Remove this member");
        let lbl_add_section = self.tr("Ajouter un membre", "Add a member");
        let lbl_add = self.tr("+ Ajouter", "+ Add");
        let lbl_leave = self.tr("🚪 Quitter le groupe", "🚪 Leave group");
        let lbl_delete = self.tr("🗑 Supprimer le groupe", "🗑 Delete group");
        let lbl_confirm = self.tr("Confirmer", "Confirm");
        let lbl_back = self.tr("Annuler", "Cancel");
        let warn_leave = self.tr(
            "Quitter ? L'historique local du salon sera effacé.",
            "Leave? The local chat history will be erased.",
        );
        let warn_delete = self.tr(
            "Supprimer le salon pour tous les membres ?",
            "Delete this group for every member?",
        );

        let confirm_state = self.group_manage_confirm;
        let mut is_open = true;
        let mut kick: Option<String> = None;
        let mut add: Option<String> = None;
        let mut set_confirm: Option<GroupConfirmAction> = None;
        let mut clear_confirm = false;
        let mut confirmed: Option<GroupConfirmAction> = None;

        egui::Window::new(format!("🔗 {}", group.name))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .resizable(false)
            .collapsible(false)
            .default_width(340.0)
            .open(&mut is_open)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.label(
                    egui::RichText::new(format!(
                        "{} {} · {} : {}",
                        lbl_created,
                        group.created_at,
                        lbl_owner,
                        display_of(&group.owner)
                    ))
                    .small()
                    .weak(),
                );
                ui.separator();

                ui.label(
                    egui::RichText::new(format!("{} ({})", lbl_members, group.members.len()))
                        .strong(),
                );
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_width(300.0);
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .id_salt("group_members")
                        .show(ui, |ui| {
                            for member in &group.members {
                                ui.horizontal(|ui| {
                                    presence_dot(ui, online_of(member));
                                    let mut label = display_of(member);
                                    if *member == group.owner {
                                        label.push_str(" 👑");
                                    }
                                    if *member == my_name {
                                        label.push(' ');
                                        label.push_str(lbl_you);
                                    }
                                    ui.label(label);
                                    // Exclusion : propriétaire uniquement,
                                    // jamais sur lui-même.
                                    if am_owner && *member != my_name {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if kick_button(ui).on_hover_text(lbl_kick).clicked()
                                                {
                                                    kick = Some(member.clone());
                                                }
                                            },
                                        );
                                    }
                                });
                            }
                        });
                });

                // Ajout de membres : pairs connus non encore membres.
                if am_owner {
                    let addable: Vec<_> = peers
                        .iter()
                        .filter(|p| !group.members.contains(&p.username))
                        .collect();
                    if !addable.is_empty() {
                        ui.separator();
                        ui.label(egui::RichText::new(lbl_add_section).strong());
                        egui::ScrollArea::vertical()
                            .max_height(120.0)
                            .id_salt("group_addable")
                            .show(ui, |ui| {
                                for peer in addable {
                                    ui.horizontal(|ui| {
                                        presence_dot(ui, peer.online);
                                        ui.label(display_of(&peer.username));
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui.small_button(lbl_add).clicked() {
                                                    add = Some(peer.username.clone());
                                                }
                                            },
                                        );
                                    });
                                }
                            });
                    }
                }

                ui.separator();
                match confirm_state {
                    Some(action) => {
                        ui.label(
                            egui::RichText::new(match action {
                                GroupConfirmAction::Leave => warn_leave,
                                GroupConfirmAction::Delete => warn_delete,
                            })
                            .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .button(egui::RichText::new(lbl_confirm).strong())
                                .clicked()
                            {
                                confirmed = Some(action);
                            }
                            if ui.button(lbl_back).clicked() {
                                clear_confirm = true;
                            }
                        });
                    }
                    None => {
                        ui.horizontal(|ui| {
                            if ui.button(lbl_leave).clicked() {
                                set_confirm = Some(GroupConfirmAction::Leave);
                            }
                            if am_owner && ui.button(lbl_delete).clicked() {
                                set_confirm = Some(GroupConfirmAction::Delete);
                            }
                        });
                    }
                }
            });

        if let Some(action) = set_confirm {
            self.group_manage_confirm = Some(action);
        }
        if clear_confirm {
            self.group_manage_confirm = None;
        }

        if let Some(user) = add {
            let outcome = {
                let mut s = self.state.lock_safe();
                // Adresses AVANT l'ajout : les membres existants reçoivent
                // l'AddMember, le nouveau reçoit l'état complet du groupe
                // (il ne connaît pas encore le salon).
                let prev_addrs = s.group_member_addrs(&group_name);
                if s.add_member_to_group(&group_name, user.clone()) {
                    let updated = s.get_group(&group_name).cloned();
                    let new_addr = s
                        .peers
                        .iter()
                        .find(|p| p.online && p.username == user)
                        .map(|p| p.addr);
                    Some((prev_addrs, updated, new_addr))
                } else {
                    None
                }
            };
            if let Some((prev_addrs, updated, new_addr)) = outcome {
                self.send_group_event(
                    &prev_addrs,
                    GroupAction::AddMember {
                        group_name: group_name.clone(),
                        username: user,
                    },
                );
                if let (Some(addr), Some(g)) = (new_addr, updated) {
                    self.send_group_event(&[addr], GroupAction::Create { group: g });
                }
            }
        }

        if let Some(user) = kick {
            let addrs = {
                let mut s = self.state.lock_safe();
                // Adresses AVANT le retrait : l'exclu est prévenu lui aussi.
                let addrs = s.group_member_addrs(&group_name);
                s.remove_member_from_group(&group_name, &user)
                    .then_some(addrs)
            };
            if let Some(addrs) = addrs {
                self.send_group_event(
                    &addrs,
                    GroupAction::RemoveMember {
                        group_name: group_name.clone(),
                        username: user,
                    },
                );
            }
        }

        if let Some(action) = confirmed {
            self.group_manage_confirm = None;
            match action {
                GroupConfirmAction::Leave => {
                    let outcome = {
                        let mut s = self.state.lock_safe();
                        let addrs = s.group_member_addrs(&group_name);
                        let me = s.my_username.clone();
                        s.leave_group(&group_name).then_some((addrs, me))
                    };
                    if let Some((addrs, me)) = outcome {
                        self.send_group_event(
                            &addrs,
                            GroupAction::RemoveMember {
                                group_name: group_name.clone(),
                                username: me,
                            },
                        );
                        self.group_manage_target = None;
                    }
                }
                GroupConfirmAction::Delete => {
                    let addrs = {
                        let mut s = self.state.lock_safe();
                        let addrs = s.group_member_addrs(&group_name);
                        s.delete_group(&group_name).then_some(addrs)
                    };
                    if let Some(addrs) = addrs {
                        self.send_group_event(
                            &addrs,
                            GroupAction::Delete {
                                group_name: group_name.clone(),
                            },
                        );
                        self.group_manage_target = None;
                    }
                }
            }
        }

        if !is_open {
            self.group_manage_target = None;
            self.group_manage_confirm = None;
        }
    }
}
