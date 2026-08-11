use super::i18n;
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
        crate::ui::theme::palette(ui).success
    } else {
        crate::ui::theme::palette(ui).danger
    };
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

impl AbcomApp {
    /// Diffuse un événement de groupe aux pairs attendus.
    fn send_group_event(&self, recipients: &[(String, std::net::SocketAddr)], action: GroupAction) {
        let event = GroupEvent { action };
        for (username, addr) in recipients {
            self.net.try_send(SendGroupRequest {
                to_peer: username.clone(),
                to_addr: *addr,
                event: event.clone(),
            });
        }
    }

    /// Modal de création de groupe : nom validé en direct (charte + doublon),
    /// sélection des membres avec présence, créateur inclus d'office.
    pub(crate) fn render_group_modal(&mut self, ctx: &egui::Context) {
        if !self.modals.group_modal_open {
            return;
        }

        let peers = self.sidebar_cache.peers.clone();
        let display_names = self.sidebar_cache.display_names.clone();
        let groups = self.sidebar_cache.groups.clone();

        // Validation vivante, avant la fenêtre : le bouton « Créer » n'est
        // actif que si tout est bon (le create_group ne peut alors échouer
        // que sur une course réseau, signalée en notification).
        let trimmed = self.modals.group_name_input.trim().to_string();
        let name_valid = !trimmed.is_empty()
            && trimmed.len() <= 50
            && trimmed
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        let duplicate = groups.iter().any(|g| g.name.eq_ignore_ascii_case(&trimmed));
        let can_create = name_valid && !duplicate;

        // Libellés résolus avant la closure.
        let title = self.t(i18n::CREER_UN_GROUPE);
        let lbl_name = self.t(i18n::NOM_DU_GROUPE);
        let hint_name = self.t(i18n::EX_EQUIPE_PROJET);
        let err_invalid = self.t(i18n::LETTRES_CHIFFRES_ET_UNIQUEMENT_50_MAX);
        let err_duplicate = self.t(i18n::CE_NOM_DE_GROUPE_EXISTE_DEJA);
        let lbl_members = self.t(i18n::MEMBRES);
        let note_self = self.t(i18n::VOUS_ETES_INCLUS_AUTOMATIQUEMENT);
        let no_peer = self.t(i18n::AUCUN_PAIR_DETECTE_SUR_LE_RESEAU);
        let sel_one = self.t(i18n::MEMBRE_SELECTIONNE);
        let sel_many = self.t(i18n::MEMBRES_SELECTIONNES);
        let btn_create = self.t(i18n::CREER_LE_GROUPE);
        let btn_cancel = self.t(i18n::ANNULER);

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
                        egui::TextEdit::singleline(&mut self.modals.group_name_input)
                            .hint_text(hint_name)
                            .desired_width(250.0),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{}/50",
                            self.modals.group_name_input.trim().len()
                        ))
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
                                .color(crate::ui::theme::palette(ui).danger),
                        );
                    } else if duplicate {
                        ui.label(
                            egui::RichText::new(err_duplicate)
                                .small()
                                .color(crate::ui::theme::palette(ui).danger),
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
                                        self.modals.group_members_selected.contains(&peer.username);
                                    ui.horizontal(|ui| {
                                        presence_dot(ui, peer.online);
                                        let label = display_names
                                            .get(idx)
                                            .cloned()
                                            .unwrap_or_else(|| peer.username.clone());
                                        if ui.checkbox(&mut selected, label).changed() {
                                            if selected {
                                                self.modals
                                                    .group_members_selected
                                                    .insert(peer.username.clone());
                                            } else {
                                                self.modals
                                                    .group_members_selected
                                                    .remove(&peer.username);
                                            }
                                        }
                                    });
                                }
                            }
                        });
                });
                let count = self.modals.group_members_selected.len();
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
            let members: Vec<String> = self.modals.group_members_selected.iter().cloned().collect();
            // Un seul passage sous verrou : création + adresses des membres,
            // envoi réseau hors verrou. (L'ancien code re-verrouillait dans le
            // corps d'un `if let` dont le garde du scrutinee vivait encore :
            // deadlock — gel de l'application — à chaque création.)
            let created = {
                let mut s = self.state.lock_safe();
                s.create_group(trimmed.clone(), members)
                    .map(|g| (g.clone(), s.group_member_recipients(&g.id)))
            };
            match created {
                Some((group, addrs)) => {
                    let conv = crate::app::AppState::group_conv_key(&group.id);
                    self.send_group_event(&addrs, GroupAction::Create { group });
                    self.modals.group_modal_open = false;
                    self.modals.group_name_input.clear();
                    self.modals.group_members_selected.clear();
                    // Ouvre directement le salon créé.
                    self.switch_conversation(Some(conv));
                }
                None => {
                    self.last_notification =
                        Some(self.t(i18n::CREATION_DU_GROUPE_IMPOSSIBLE).to_string());
                    self.notification_time = std::time::Instant::now();
                }
            }
        }

        if do_cancel || !is_open {
            self.modals.group_modal_open = false;
            self.modals.group_name_input.clear();
            self.modals.group_members_selected.clear();
        }
    }

    /// Modal de gestion d'un salon : liste des membres (présence, couronne du
    /// propriétaire), ajout et exclusion (propriétaire), départ (tous),
    /// suppression (propriétaire) — actions destructrices confirmées en
    /// deux temps.
    pub(crate) fn render_group_manage_modal(&mut self, ctx: &egui::Context) {
        let Some(group_id) = self.modals.group_manage_target.clone() else {
            return;
        };
        // Groupe disparu entre-temps (suppression ou exclusion reçue du
        // réseau) : fermer le modal sans rien afficher.
        let Some(group) = self
            .sidebar_cache
            .groups
            .iter()
            .find(|g| g.id == group_id)
            .cloned()
        else {
            self.modals.group_manage_target = None;
            self.modals.group_manage_confirm = None;
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
        let lbl_owner = self.t(i18n::PROPRIETAIRE);
        let lbl_created = self.t(i18n::CREE_LE);
        let lbl_members = self.t(i18n::MEMBRES);
        let lbl_you = self.t(i18n::VOUS);
        let lbl_kick = self.t(i18n::EXCLURE_CE_MEMBRE);
        let lbl_add_section = self.t(i18n::AJOUTER_UN_MEMBRE);
        let lbl_add = self.t(i18n::AJOUTER);
        let lbl_leave = self.t(i18n::QUITTER_LE_GROUPE);
        let lbl_delete = self.t(i18n::SUPPRIMER_LE_GROUPE);
        let lbl_confirm = self.t(i18n::CONFIRMER);
        let lbl_back = self.t(i18n::ANNULER);
        let warn_leave = self.t(i18n::QUITTER_L_HISTORIQUE_LOCAL_DU_SALON);
        let warn_delete = self.t(i18n::SUPPRIMER_LE_SALON_POUR_TOUS_LES);

        let confirm_state = self.modals.group_manage_confirm;
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
                            .color(crate::ui::theme::palette(ui).danger),
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
            self.modals.group_manage_confirm = Some(action);
        }
        if clear_confirm {
            self.modals.group_manage_confirm = None;
        }

        if let Some(user) = add {
            let outcome = {
                let mut s = self.state.lock_safe();
                // Adresses AVANT l'ajout : les membres existants reçoivent
                // l'AddMember, le nouveau reçoit l'état complet du groupe
                // (il ne connaît pas encore le salon).
                let prev_addrs = s.group_member_recipients(&group_id);
                if s.add_member_to_group(&group_id, user.clone()) {
                    let updated = s.get_group(&group_id).cloned();
                    let new_addr = s
                        .peers
                        .iter()
                        .find(|p| p.online && p.username == user)
                        .map(|p| (p.username.clone(), p.addr));
                    Some((prev_addrs, updated, new_addr))
                } else {
                    None
                }
            };
            if let Some((prev_addrs, updated, new_addr)) = outcome {
                self.send_group_event(
                    &prev_addrs,
                    GroupAction::AddMember {
                        group_id: group_id.clone(),
                        username: user,
                    },
                );
                if let (Some(recipient), Some(g)) = (new_addr, updated) {
                    self.send_group_event(&[recipient], GroupAction::Create { group: g });
                }
            }
        }

        if let Some(user) = kick {
            let addrs = {
                let mut s = self.state.lock_safe();
                // Adresses AVANT le retrait : l'exclu est prévenu lui aussi.
                let addrs = s.group_member_recipients(&group_id);
                s.remove_member_from_group(&group_id, &user)
                    .then_some(addrs)
            };
            if let Some(addrs) = addrs {
                self.send_group_event(
                    &addrs,
                    GroupAction::RemoveMember {
                        group_id: group_id.clone(),
                        username: user,
                    },
                );
            }
        }

        if let Some(action) = confirmed {
            self.modals.group_manage_confirm = None;
            match action {
                GroupConfirmAction::Leave => {
                    let outcome = {
                        let mut s = self.state.lock_safe();
                        let addrs = s.group_member_recipients(&group_id);
                        let me = s.my_username.clone();
                        s.leave_group(&group_id).then_some((addrs, me))
                    };
                    if let Some((addrs, me)) = outcome {
                        self.send_group_event(
                            &addrs,
                            GroupAction::RemoveMember {
                                group_id: group_id.clone(),
                                username: me,
                            },
                        );
                        self.modals.group_manage_target = None;
                    }
                }
                GroupConfirmAction::Delete => {
                    let addrs = {
                        let mut s = self.state.lock_safe();
                        let addrs = s.group_member_recipients(&group_id);
                        s.delete_group(&group_id).then_some(addrs)
                    };
                    if let Some(addrs) = addrs {
                        self.send_group_event(
                            &addrs,
                            GroupAction::Delete {
                                group_id: group_id.clone(),
                            },
                        );
                        self.modals.group_manage_target = None;
                    }
                }
            }
        }

        if !is_open {
            self.modals.group_manage_target = None;
            self.modals.group_manage_confirm = None;
        }
    }
}
