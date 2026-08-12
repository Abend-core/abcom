use crate::ui::i18n;
use eframe::egui;

use crate::util::MutexExt;

use super::{AbcomApp, ReplyTarget};

mod row;
mod toolbar;

#[cfg(test)]
pub(crate) use row::GROUP_BREAK_SECS;
use row::{
    apply_media_action, render_day_divider, render_message_body, render_message_header,
    render_reaction_pills, render_reply_quote, AVATAR_GUTTER, AVATAR_SIZE, GROUP_SPACING,
    HIGHLIGHT_SECS, MESSAGE_RIGHT_MARGIN,
};
pub(crate) use row::{
    day_divider_label, header_time, message_day, peer_color_for, starts_new_group,
};

impl AbcomApp {
    /// Zone centrale : fil de la conversation sélectionnée. Le rendu
    /// consomme exclusivement le cache dérivé (`ui/snapshot.rs`) : aucun
    /// verrou sur `AppState`, aucun clone de conversation, aucun re-parse
    /// markdown par frame. Le fil est fenêtré façon Discord : seuls les
    /// derniers messages sont rendus, remonter charge les 100 précédents.
    pub(crate) fn show_central_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        egui::CentralPanel::default().show(ui, |ui| {
            let rows = self.chat_cache.rows.clone();
            let my_name = self.chat_cache.my_name.clone();
            let selected_conv: Option<String> = self.chat_cache.conversation().map(str::to_string);
            let private_peer = selected_conv
                .as_deref()
                .filter(|c| !c.starts_with('#'))
                .map(str::to_string);
            let is_broadcast = selected_conv.is_none();

            // Salon sélectionné : la clé porte l'identifiant, jamais le libellé.
            let selected_group_id = selected_conv
                .as_deref()
                .and_then(|c| c.strip_prefix('#'))
                .map(str::to_string);
            let selected_group = selected_group_id
                .as_deref()
                .and_then(|id| self.sidebar_cache.groups.iter().find(|g| g.id == id));
            let conversation_title = match (&self.chat_cache.private_peer_display, selected_group) {
                (Some(name), _) => name.clone(),
                // Le titre affiche le nom du salon, pas son identifiant.
                (None, Some(group)) => group.name.clone(),
                (None, None) => selected_conv
                    .clone()
                    .unwrap_or_else(|| self.t(i18n::TOUS_2).to_string()),
            };
            // Sous-titre « N membres » sous le titre.
            let group_subtitle = selected_group.map(|g| {
                let n = g.members.len();
                if n > 1 {
                    format!("{} {}", n, self.t(i18n::MEMBRES_2))
                } else {
                    format!("{} {}", n, self.t(i18n::MEMBRE))
                }
            });

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.heading(&conversation_title);
                    if let Some(subtitle) = &group_subtitle {
                        ui.label(egui::RichText::new(subtitle).small().weak());
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button(self.t(i18n::ACTIONS), |ui| {
                        let sound_text = if self.enable_sound_notifications {
                            self.t(i18n::DESACTIVER_TOUS_LES_SONS)
                        } else {
                            self.t(i18n::ACTIVER_TOUS_LES_SONS)
                        };
                        if ui.button(sound_text).clicked() {
                            self.enable_sound_notifications = !self.enable_sound_notifications;
                            ui.close();
                        }
                        let this_conv = selected_conv.clone();
                        let is_muted = self.muted_conversations.contains(&this_conv);
                        let mute_text = if is_muted {
                            self.t(i18n::REACTIVER_LES_SONS_DE_CE_SALON)
                        } else {
                            self.t(i18n::MUET_POUR_CE_SALON)
                        };
                        if ui.button(mute_text).clicked() {
                            if is_muted {
                                self.muted_conversations.remove(&this_conv);
                            } else {
                                self.muted_conversations.insert(this_conv);
                            }
                            ui.close();
                        }
                        if ui.button(self.t(i18n::VOIR_LES_PARTICIPANTS)).clicked() {
                            self.modals.participants_open = true;
                            ui.close();
                        }
                        if let Some(gid) = &selected_group_id {
                            if ui.button(self.t(i18n::GERER_LE_GROUPE)).clicked() {
                                self.modals.group_manage_target = Some(gid.clone());
                                self.modals.group_manage_confirm = None;
                                ui.close();
                            }
                        }
                        if let Some(user) = &private_peer {
                            if ui.button(self.t(i18n::RENOMMER_CE_CONTACT)).clicked() {
                                self.modals.rename_input = self
                                    .state
                                    .lock()
                                    .unwrap()
                                    .peer_records
                                    .iter()
                                    .find(|r| &r.username == user)
                                    .and_then(|r| r.alias.clone())
                                    .unwrap_or_default();
                                self.modals.rename_target = Some(user.clone());
                                ui.close();
                            }
                        }
                        if !is_broadcast && ui.button(self.t(i18n::EFFACER_L_HISTORIQUE)).clicked()
                        {
                            self.state.lock_safe().clear_conversation_history();
                            ui.close();
                        }
                    });
                });
            });
            ui.separator();

            // Popup participants (instantané depuis le cache latéral).
            if self.modals.participants_open {
                let my_name2 = self.sidebar_cache.my_username.clone();
                let sel_conv = self.sidebar_cache.selected_conversation.clone();
                let peers = self.sidebar_cache.peers.clone();
                // Salon : membres réels du groupe (pas la liste des pairs).
                let group_view = sel_conv
                    .as_deref()
                    .and_then(|c| c.strip_prefix('#'))
                    .and_then(|id| {
                        self.sidebar_cache
                            .groups
                            .iter()
                            .find(|g| g.id == id)
                            .cloned()
                    });
                // Un salon s'affiche par son nom : la clé porte un identifiant
                // qui n'a aucun sens pour l'utilisateur.
                let conv_name = match (&group_view, &sel_conv) {
                    (Some(group), _) => format!("#{}", group.name),
                    (None, Some(conv)) => conv.clone(),
                    (None, None) => self.t(i18n::TOUS_2).to_string(),
                };
                let mut open = self.modals.participants_open;
                egui::Window::new(self.t(i18n::PARTICIPANTS))
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {}",
                                self.t(i18n::CONVERSATION),
                                conv_name
                            ))
                            .strong(),
                        );
                        ui.separator();
                        if sel_conv.is_none() {
                            for peer in peers.iter() {
                                ui.label(&peer.username);
                            }
                            if peers.is_empty() {
                                ui.label(self.t(i18n::AUCUN_PARTICIPANT_CONNECTE));
                            }
                        } else if let Some(group) = &group_view {
                            // Membres du salon : présence, couronne du
                            // propriétaire, « (vous) » pour soi.
                            for member in &group.members {
                                let online = *member == my_name2
                                    || peers.iter().any(|p| p.username == *member && p.online);
                                let dot = if online { "🟢" } else { "🔴" };
                                let mut line = format!("{dot} {member}");
                                if *member == group.owner {
                                    line.push_str(" 👑");
                                }
                                if *member == my_name2 {
                                    line.push(' ');
                                    line.push_str(self.t(i18n::VOUS));
                                }
                                ui.label(line);
                            }
                        } else {
                            ui.label(format!("{} ({})", my_name2, self.t(i18n::VOUS_3)));
                            if let Some(peer) = sel_conv {
                                ui.label(&peer);
                            }
                        }
                    });
                self.modals.participants_open = open;
            }

            // Modale de renommage de contact
            if let Some(target) = self.modals.rename_target.clone() {
                // Libellés calculés avant la closure (évite d'emprunter `self`
                // pendant qu'on édite `self.modals.rename_input`).
                let title = self.t(i18n::RENOMMER_LE_CONTACT);
                let lbl_original = self.t(i18n::NOM_D_ORIGINE);
                let hint = self.t(i18n::ALIAS_VIDE_RETIRER);
                let save_lbl = self.t(i18n::ENREGISTRER);
                let clear_lbl = self.t(i18n::RETIRER_L_ALIAS);

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
                            egui::TextEdit::singleline(&mut self.modals.rename_input)
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
                    let trimmed = self.modals.rename_input.trim();
                    let alias = (!trimmed.is_empty()).then(|| trimmed.to_string());
                    self.state.lock_safe().set_peer_alias(&target, alias);
                    self.modals.rename_target = None;
                } else if do_clear {
                    self.state.lock_safe().set_peer_alias(&target, None);
                    self.modals.rename_target = None;
                } else if !open {
                    self.modals.rename_target = None;
                }
            }

            // Alerte TOFU : la clé présentée par un pair ne correspond plus à
            // celle épinglée. Deux causes possibles — usurpation, ou
            // réinstallation légitime du pair. On ne tranche pas à sa place :
            // la connexion reste refusée tant que l'utilisateur n'a pas
            // explicitement ré-appairé, empreinte vérifiée hors-bande.
            if let Some((peer, offered_key)) = self.modals.key_mismatch.clone() {
                let title = self.t(i18n::CLE_D_IDENTITE_MODIFIEE);
                let explain = self.t(i18n::LA_CLE_DE_CE_PAIR_NE);
                let trust_lbl = self.t(i18n::FAIRE_CONFIANCE_A_LA_NOUVELLE_CLE);
                let keep_lbl = self.t(i18n::GARDER_L_ANCIENNE_CLE);

                // `Modal` plutôt que `Window` : voile de fond et focus piégé —
                // cette alerte ne doit pas pouvoir être ignorée par inadvertance.
                let modal = egui::Modal::new(egui::Id::new("key_mismatch")).show(ctx, |ui| {
                    ui.set_max_width(360.0);
                    ui.heading(format!("⚠️ {title}"));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(&peer).strong());
                    ui.add_space(6.0);
                    ui.label(explain);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        (
                            ui.button(trust_lbl).clicked(),
                            ui.button(keep_lbl).clicked(),
                        )
                    })
                    .inner
                });
                let (do_trust, do_keep) = modal.inner;
                // Clic hors modale ou Échap : on garde la clé actuelle, jamais
                // le contraire — refuser est toujours le choix sûr.
                let dismissed = modal.backdrop_response.clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape));

                if do_trust {
                    // On épingle la clé effectivement présentée, pas la
                    // prochaine venue : sinon une autre machine du réseau
                    // pourrait se glisser dans la fenêtre de ré-appairage.
                    self.trust.repin(&peer, &offered_key);
                    self.last_notification =
                        Some(self.t(i18n::NOUVELLE_CLE_ACCEPTEE_POUR_CE_PAIR).to_string());
                    self.notification_time = std::time::Instant::now();
                    self.modals.key_mismatch = None;
                } else if do_keep || dismissed {
                    self.modals.key_mismatch = None;
                }
            }

            // Avatars et textures des médias image, préparés hors zone
            // défilante (les deux caches de textures sont persistants :
            // seuls les éléments manquants déclenchent un chargement).
            let authors = self.chat_cache.authors.clone();
            let mut author_avatars: std::collections::HashMap<String, Option<egui::TextureHandle>> =
                std::collections::HashMap::new();
            for author in &authors {
                let texture = self.avatar_texture(ctx, author);
                author_avatars.insert(author.clone(), texture);
            }
            let media_ids = self.chat_cache.image_media_ids.clone();
            let mut media_textures: std::collections::HashMap<String, Option<egui::TextureHandle>> =
                std::collections::HashMap::new();
            for id in &media_ids {
                let texture = self.media_texture(ctx, id);
                media_textures.insert(id.clone(), texture);
            }
            // Présence sur disque des pièces jointes du fil : un seul `stat`
            // par média et par session (le résultat est mémorisé), jamais un
            // accès disque par frame.
            let local_ids = self.chat_cache.local_media_ids.clone();
            let media_present = self.media_presence(&local_ids);

            // Actions médias collectées pendant le rendu, appliquées ensuite.
            let mut media_view_open: Option<String> = None;
            let mut media_download: Option<(String, String)> = None;

            // Fenêtrage : indice du premier message rendu.
            let total = rows.len();
            let mut start = total.saturating_sub(self.chat_visible_count);
            // Un saut vers un message hors fenêtre l'étend jusqu'à lui.
            if let Some(target) = self.scroll_to_message {
                if let Some(idx) = rows.iter().position(|r| r.hash == target) {
                    if idx < start {
                        start = idx;
                        self.chat_visible_count = total - idx;
                    }
                }
            }

            let not_found_label = self.t(i18n::MESSAGE_D_ORIGINE_INTROUVABLE);
            let reply_label = self.t(i18n::REPONDRE);
            let add_reaction_label = self.t(i18n::AJOUTER_UNE_REACTION);
            let language = self.ui_language;

            // Largeur du fil figée AVANT la zone défilante, depuis la largeur
            // du panneau (stable, ne bouge qu'au redimensionnement) et arrondie
            // au pixel : évite le tremblement de la marge droite qu'on avait en
            // recalculant `available_width` à l'intérieur du scroll chaque frame.
            let thread_width = (ui.available_width() - MESSAGE_RIGHT_MARGIN)
                .max(120.0)
                .floor();

            // Aire de messages. Le collage au bas est suspendu quand un saut
            // vers un message est en attente : sinon il écrase le
            // `scroll_to_rect` du saut et le fil reste en bas.
            let scroll_out = egui::ScrollArea::vertical()
                .id_salt("chat_scroll")
                .auto_shrink([false; 2])
                .stick_to_bottom(self.scroll_to_message.is_none())
                .show(ui, |ui| {
                    // Marge à droite : largeur figée (cf. thread_width), rien ne
                    // colle au bord et la marge ne tremble plus.
                    ui.set_max_width(thread_width);

                    if rows.is_empty() {
                        ui.add_space(50.0);
                        ui.label(egui::RichText::new(self.t(i18n::AUCUN_MESSAGE)).weak());
                    }

                    // Une seule ligne peut revendiquer le survol par frame :
                    // les rects de survol débordent de 2 px sur leurs voisins
                    // (couverture des interstices), sans ce verrou deux lignes
                    // adjacentes pouvaient se surligner en même temps.
                    let mut hover_claimed = false;

                    for (i, row) in rows[start..].iter().enumerate() {
                        let msg = &row.msg;
                        let hash = row.hash;
                        // Index absolu dans le fil : désambiguïse les messages
                        // au hash identique (anciens messages sans nonce) pour
                        // tout ce qui est purement visuel (survol, barre
                        // d'actions, identifiants egui).
                        let abs_idx = start + i;

                        // Première ligne d'une fenêtre tronquée : séparateur
                        // de date forcé (situe la coupure) et en-tête forcé
                        // (pas de continuation orpheline sans avatar).
                        let window_head = i == 0 && start > 0;
                        let divider = row.day_divider.as_ref().or(if window_head {
                            row.day_label.as_ref()
                        } else {
                            None
                        });
                        if let Some(label) = divider {
                            render_day_divider(ui, label);
                        }
                        let starts_group = row.starts_group || window_head;

                        let mut reaction_clicked: Option<String> = None;
                        let mut reply_quote_clicked = false;
                        let mut collapse_toggled = false;
                        let expanded = self.expanded_messages.contains(&hash);

                        // Fond pleine largeur de la ligne (survol / flash de
                        // surlignage), inséré sous le contenu et rempli après
                        // le rendu, une fois l'état de survol connu.
                        let row_bg = ui.painter().add(egui::Shape::Noop);

                        let reply_avatar = row.reply.as_ref().and_then(|r| {
                            r.resolved
                                .as_ref()
                                .and_then(|m| author_avatars.get(&m.from))
                                .and_then(|t| t.as_ref())
                        });
                        let reply_media_tex = row.reply.as_ref().and_then(|r| {
                            r.resolved
                                .as_ref()
                                .and_then(|m| m.media.as_ref())
                                .and_then(|med| {
                                    media_textures.get(&med.id).and_then(|t| t.as_ref())
                                })
                        });

                        let mut gutter_rect: Option<egui::Rect> = None;
                        let row_resp = if starts_group {
                            ui.add_space(GROUP_SPACING);
                            ui.vertical(|ui| {
                                if let Some(reply) = &row.reply {
                                    if render_reply_quote(
                                        ui,
                                        abs_idx,
                                        reply.resolved.as_ref(),
                                        &reply.author,
                                        reply.author_color,
                                        reply_avatar,
                                        reply_media_tex,
                                        not_found_label,
                                    ) {
                                        reply_quote_clicked = true;
                                    }
                                }
                                ui.horizontal(|ui| {
                                    // Retrait du texte = avatar + gouttière, sans
                                    // espacement parasite, pour qu'il coïncide avec
                                    // les messages de continuation (cf. branche else).
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    let avatar = author_avatars
                                        .get(&msg.from)
                                        .and_then(|texture| texture.as_ref());
                                    super::avatar::show_avatar(
                                        ui,
                                        avatar,
                                        &row.display_name,
                                        AVATAR_SIZE,
                                    );
                                    ui.add_space(AVATAR_GUTTER);
                                    ui.vertical(|ui| {
                                        render_message_header(
                                            ui,
                                            &row.display_name,
                                            &row.header_time,
                                            row.name_color,
                                            row.receipt,
                                            row.receipt_detail.as_ref(),
                                            row.hash,
                                            language,
                                        );
                                        let (media_action, toggled) = render_message_body(
                                            ui,
                                            msg,
                                            &row.markdown,
                                            row.collapse.as_ref(),
                                            expanded,
                                            language,
                                            &self.emoji.map,
                                            &self.emoji.textures,
                                            &media_textures,
                                            &self.media.progress,
                                            &media_present,
                                        );
                                        collapse_toggled |= toggled;
                                        if let Some(action) = media_action {
                                            apply_media_action(
                                                action,
                                                msg,
                                                &mut media_view_open,
                                                &mut media_download,
                                            );
                                        }
                                        if let Some(emoji) = render_reaction_pills(
                                            ui,
                                            &row.reactions,
                                            &my_name,
                                            &self.emoji.map,
                                            &self.emoji.textures,
                                        ) {
                                            reaction_clicked = Some(emoji);
                                        }
                                    });
                                });
                            })
                        } else {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(AVATAR_SIZE + AVATAR_GUTTER, 20.0),
                                    egui::Sense::hover(),
                                );
                                // L'heure est peinte après le rendu de la
                                // ligne, dès que celle-ci est survolée.
                                gutter_rect = Some(rect);
                                ui.vertical(|ui| {
                                    let (media_action, toggled) = render_message_body(
                                        ui,
                                        msg,
                                        &row.markdown,
                                        row.collapse.as_ref(),
                                        expanded,
                                        language,
                                        &self.emoji.map,
                                        &self.emoji.textures,
                                        &media_textures,
                                        &self.media.progress,
                                        &media_present,
                                    );
                                    collapse_toggled |= toggled;
                                    if let Some(action) = media_action {
                                        apply_media_action(
                                            action,
                                            msg,
                                            &mut media_view_open,
                                            &mut media_download,
                                        );
                                    }
                                    if let Some(emoji) = render_reaction_pills(
                                        ui,
                                        &row.reactions,
                                        &my_name,
                                        &self.emoji.map,
                                        &self.emoji.textures,
                                    ) {
                                        reaction_clicked = Some(emoji);
                                    }
                                });
                            })
                        };
                        // Rectangle pleine largeur de la ligne : comme sur
                        // Discord, le survol et le fond couvrent tout le fil,
                        // pas seulement la largeur du texte.
                        let row_rect = egui::Rect::from_x_y_ranges(
                            ui.max_rect().x_range(),
                            row_resp.response.rect.y_range(),
                        )
                        .expand2(egui::vec2(0.0, 2.0));

                        // Saut demandé vers ce message (clic sur une citation).
                        if self.scroll_to_message == Some(hash) {
                            ui.scroll_to_rect(row_rect, Some(egui::Align::Center));
                            self.scroll_to_message = None;
                            self.highlight_message = Some((hash, std::time::Instant::now()));
                        }

                        // Survol : barre d'actions flottante (emojis récents,
                        // "+", répondre). Reste affichée tant que le pointeur
                        // est sur la ligne ou sur la barre elle-même, pour
                        // éviter tout clignotement en s'y déplaçant.
                        let row_hovered = !hover_claimed && ui.rect_contains_pointer(row_rect);
                        if row_hovered {
                            self.hover_toolbar_target = Some((abs_idx, hash));
                            hover_claimed = true;
                        }
                        // Ligne « active » : survolée, ou pointeur sur sa
                        // barre d'actions flottante.
                        let row_active = self.hover_toolbar_target == Some((abs_idx, hash));

                        // Heure dans la gouttière des messages de continuation,
                        // visible dès que la ligne est survolée (pas seulement
                        // la gouttière), et centrée verticalement sur le
                        // message — reste en face du contenu même pour un
                        // grand média ou un long texte.
                        if row_active {
                            if let Some(rect) = gutter_rect {
                                ui.painter().text(
                                    egui::pos2(rect.center().x, row_rect.center().y),
                                    egui::Align2::CENTER_CENTER,
                                    &row.header_time,
                                    egui::TextStyle::Small.resolve(ui.style()),
                                    crate::ui::theme::palette(ui).text_muted,
                                );
                            }
                        }

                        // Fond de la ligne : flash de surlignage qui s'estompe
                        // après un saut, sinon grisé de survol façon Discord.
                        let highlight_elapsed = self
                            .highlight_message
                            .filter(|(h, _)| *h == hash)
                            .map(|(_, since)| since.elapsed().as_secs_f32());
                        if let Some(elapsed) = highlight_elapsed {
                            if elapsed < HIGHLIGHT_SECS {
                                let alpha = ((1.0 - elapsed / HIGHLIGHT_SECS) * 44.0) as u8;
                                ui.painter().set(
                                    row_bg,
                                    egui::Shape::rect_filled(
                                        row_rect,
                                        0.0,
                                        egui::Color32::from_rgba_unmultiplied(88, 101, 242, alpha),
                                    ),
                                );
                                ui.ctx().request_repaint();
                            } else {
                                self.highlight_message = None;
                            }
                        } else if row_active {
                            let tint = if ui.visuals().dark_mode {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 7)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 10)
                            };
                            ui.painter()
                                .set(row_bg, egui::Shape::rect_filled(row_rect, 0.0, tint));
                        }

                        let mut reply_requested = false;
                        if row_active {
                            let result = self.show_hover_toolbar(
                                ctx,
                                abs_idx,
                                hash,
                                row_rect,
                                reply_label,
                                add_reaction_label,
                            );
                            if !row_hovered && !result.pointer_over_toolbar {
                                self.hover_toolbar_target = None;
                            }
                            if let Some(emoji) = result.quick_emoji {
                                reaction_clicked = Some(emoji);
                            }
                            reply_requested = result.reply_clicked;
                        }

                        if collapse_toggled && !self.expanded_messages.remove(&hash) {
                            self.expanded_messages.insert(hash);
                        }
                        if let Some(emoji) = reaction_clicked {
                            self.send_reaction(hash, &emoji);
                        }
                        if reply_quote_clicked {
                            if let Some(target) = msg.reply_to {
                                self.scroll_to_message = Some(target);
                                ctx.request_repaint();
                            }
                        }
                        if reply_requested {
                            self.replying_to = Some(ReplyTarget {
                                message_hash: hash,
                                author: row.display_name.clone(),
                                content_snippet: super::media::elide(&msg.content, 80),
                                media_thumb: msg.media.clone(),
                            });
                        }
                    }

                    // Offres de médias volumineux (au-delà du seuil d'accord) à accepter/refuser.
                    self.render_media_offers(ui);
                });

            // Pagination façon Discord : arrivé près du haut, charger les 100
            // messages précédents — d'abord depuis la fenêtre mémoire, puis
            // depuis SQLite quand elle est épuisée — et compenser l'offset de
            // la hauteur ajoutée (aucun saut visuel, pas de bouton).
            if let Some(prev_height) = self.chat_prepend_fix {
                let delta = scroll_out.content_size.y - prev_height;
                if delta > 0.0 {
                    // Le contenu ajouté est arrivé : compenser l'offset, puis
                    // jeter la frame courante (rendue avec l'ancien offset) et
                    // re-rendre immédiatement — sans ça, une frame décalée
                    // s'affiche à chaque lot chargé (tremblement du fil).
                    self.chat_prepend_fix = None;
                    let mut state = scroll_out.state;
                    state.offset.y += delta;
                    state.store(ctx, scroll_out.id);
                    ctx.request_discard("chat prepend anchor");
                }
                // delta == 0 : requête SQLite encore en vol, on attend.
            } else if scroll_out.state.offset.y < 400.0 && !rows.is_empty() {
                if start > 0 {
                    self.chat_visible_count =
                        (self.chat_visible_count + super::CHAT_WINDOW_STEP).min(total);
                    self.chat_prepend_fix = Some(scroll_out.content_size.y);
                    ctx.request_repaint();
                } else if !self.loading_older && self.state.lock_safe().request_older_messages() {
                    self.loading_older = true;
                    self.chat_prepend_fix = Some(scroll_out.content_size.y);
                }
            }

            // Application des actions médias collectées pendant le rendu.
            if let Some(id) = media_view_open {
                self.media.viewer = Some(id);
            }
            if let Some((id, filename)) = media_download {
                self.download_media(&id, &filename);
            }
        });
    }

    /// Bandeaux d'acceptation des médias volumineux (au-delà du seuil d'accord) reçus. Accepter →
    /// le pair streame alors le média ; Refuser → l'envoi est abandonné.
    fn render_media_offers(&mut self, ui: &mut egui::Ui) {
        if self.media.pending_offers.is_empty() {
            return;
        }
        let mut decided: Option<(usize, bool)> = None;

        for (index, offer) in self.media.pending_offers.iter().enumerate() {
            ui.add_space(6.0);
            egui::Frame::group(ui.style())
                .fill(crate::ui::theme::palette(ui).surface_hover)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            offer.from,
                            self.t(i18n::SOUHAITE_VOUS_ENVOYER_UN_FICHIER)
                        ))
                        .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} ({})",
                            offer.filename,
                            format_bytes(offer.size_bytes)
                        ))
                        .small(),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(self.t(i18n::REFUSER)).clicked() {
                            decided = Some((index, false));
                        }
                        if ui.button(self.t(i18n::ACCEPTER)).clicked() {
                            decided = Some((index, true));
                        }
                    });
                });
        }

        if let Some((index, accept)) = decided {
            let offer = self.media.pending_offers.remove(index);
            if !accept {
                // Refus : annoter le fil (message attribué à l'expéditeur).
                let mut s = self.state.lock_safe();
                let me = s.my_username.clone();
                s.add_message(super::media::refused_media_message(
                    &offer.from,
                    &offer.filename,
                    Some(me),
                ));
            }
            let _ = offer.decision_tx.send(accept);
        }
    }

    /// Popup de notification en haut à droite
    pub(crate) fn show_notification(&mut self, ctx: &egui::Context) {
        if let Some(notif) = &self.last_notification {
            if self.notification_time.elapsed().as_secs_f32() < 3.0 {
                egui::Window::new(self.t(i18n::NOTIFICATION))
                    .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
                    .resizable(false)
                    .collapsible(false)
                    .title_bar(false)
                    .show(ctx, |ui| {
                        ui.colored_label(
                            crate::ui::theme::palette(ui).accent_soft,
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
pub(crate) fn format_bytes(bytes: u64) -> String {
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

/// Dessine une ou deux coches selon le statut de lecture du message.
/// `read = false` → une coche grise (envoyé) ; `read = true` → deux coches bleues (lu).
/// Affiche l'indicateur de statut d'un message :
/// - ✓  gris  = envoyé, livraison en attente
/// - ✓✓ gris  = livré (ACK reçu), pas encore lu
/// - ✓✓ bleu  = lu (ReadReceipt reçu)
fn show_receipt(ui: &mut egui::Ui, delivered: bool, read: bool, failed: bool) {
    if failed {
        ui.label(
            egui::RichText::new("!")
                .strong()
                .color(crate::ui::theme::palette(ui).danger),
        )
        .on_hover_text("Échec de livraison / Delivery failed");
        return;
    }
    let double = delivered || read;
    let w = if double { 17.0_f32 } else { 9.0_f32 };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(w, 12.0), egui::Sense::hover());
    // Les coches sont peintes : sans libellé, l'état de livraison n'existe
    // pas pour un lecteur d'écran.
    let state = if read {
        "Lu"
    } else if delivered {
        "Reçu"
    } else {
        "Envoyé"
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, state));
    if !ui.is_rect_visible(rect) {
        return;
    }
    let color = if read {
        crate::ui::theme::palette(ui).receipt_read // bleu = lu
    } else {
        crate::ui::theme::palette(ui).text_muted // gris = envoyé ou livré
    };
    let stroke = egui::Stroke::new(1.5, color);
    let p = ui.painter();

    let draw_tick = |ox: f32| {
        let base = rect.left_top() + egui::vec2(ox, 4.0);
        p.line_segment([base, base + egui::vec2(2.5, 3.0)], stroke);
        p.line_segment(
            [base + egui::vec2(2.5, 3.0), base + egui::vec2(8.0, -1.5)],
            stroke,
        );
    };

    draw_tick(0.0);
    if double {
        draw_tick(6.0);
    }
}

#[cfg(test)]
#[path = "../../tests/test_ui_chat_panel.rs"]
mod tests;
