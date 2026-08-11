use crate::message::{TypingIndicator, TypingRequest};
use crate::ui::i18n;
use crate::util::MutexExt;
use eframe::egui;

use super::composer;
use super::emoji_picker::emoji_shortcode_trigger;
use super::AbcomApp;

mod sending;
mod widgets;

#[cfg(test)]
use sending::chat_wire_size;
use sending::send_current_message;
use widgets::{
    action_button, attachment_chip, attachment_menu_popup, chip_remove_button, icon_button,
    paint_plus_icon, paint_send_icon, should_send_message, AttachmentMenuAction,
    ACTION_BUTTON_SIZE,
};
#[cfg(test)]
use widgets::{attachment_label, push_unique_paths};

impl AbcomApp {
    /// Collage trop long → pièce jointe `.txt` en 0600 dans `scratch/`, purgée après 24 h.
    fn stash_overflow_paste(&mut self, text: &str) {
        crate::config::purge_scratch();
        let filename = format!(
            "texte-colle-{}.txt",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        let path = match crate::config::scratch_dir() {
            Ok(dir) => dir.join(filename),
            Err(err) => {
                self.last_notification = Some(format!(
                    "{} : {err}",
                    self.t(i18n::IMPOSSIBLE_D_ECRIRE_LE_TEXTE_COLLE)
                ));
                self.notification_time = std::time::Instant::now();
                return;
            }
        };
        match std::fs::write(&path, text) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                }
                self.composer.pending_attachments.push(path);
                self.last_notification =
                    Some(self.t(i18n::TEXTE_COLLE_TROP_LONG_JOINT_EN).to_string());
            }
            Err(err) => {
                self.last_notification = Some(format!(
                    "{} : {err}",
                    self.t(i18n::IMPOSSIBLE_D_ECRIRE_LE_TEXTE_COLLE)
                ));
            }
        }
        self.notification_time = std::time::Instant::now();
    }

    /// Barre de saisie en bas de fenêtre. Retourne `(emoji_cliqué, gif_cliqué)`
    /// pour piloter l'ouverture des sélecteurs respectifs.
    pub(crate) fn show_input_bar(&mut self, ui: &mut egui::Ui) -> (bool, bool) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        // Présence et frappe lues depuis le cache dérivé : aucune prise de
        // verrou par frame dans la barre de saisie.
        let selected_peer_online = self.sidebar_cache.selected_peer_online;

        // Hors ligne, on informe mais on **laisse écrire** : le message part
        // dans la file d'attente et sera livré à la reconnexion du pair.
        if !selected_peer_online {
            egui::Panel::bottom("offline_notice")
                .exact_size(22.0)
                .show(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new(self.t(i18n::HORS_LIGNE_VOTRE_MESSAGE_PARTIRA_A))
                                .color(crate::ui::theme::palette(ui).danger)
                                .small(),
                        );
                    });
                });
        }

        let mut emoji_button_clicked = false;
        let mut gif_button_clicked = false;
        let mut picker_action: Option<AttachmentMenuAction> = None;
        let typing_list = self.sidebar_cache.typing.clone();
        let add_files_label = self.t(i18n::AJOUTER_DES_FICHIERS);
        let add_folder_label = self.t(i18n::AJOUTER_UN_DOSSIER);

        // Marge uniforme entre les bords du panneau et le cadre du composant
        // (la marge par défaut du panneau est plus large sur les côtés).
        egui::Panel::bottom("input_panel")
            .resizable(false)
            .frame(egui::Frame::side_top_panel(&ui.style().clone()).inner_margin(egui::Margin::same(8)))
            .show(ui, |ui| {
                let gif_label = self.t(i18n::GIF);
                egui::Frame::default()
                    .fill(crate::ui::theme::palette(ui).surface_strong)
                    .stroke(egui::Stroke::new(1.0, crate::ui::theme::palette(ui).separator))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            // Aperçu de réponse : extrait les données possédées
                            // avant tout appel `&mut self` (chargement de
                            // texture), pour ne pas garder `self.replying_to`
                            // emprunté pendant l'appel.
                            let reply_preview = self.replying_to.as_ref().map(|r| {
                                (
                                    r.author.clone(),
                                    r.content_snippet.clone(),
                                    r.media_thumb.clone(),
                                )
                            });
                            if let Some((author, snippet, media)) = reply_preview {
                                let reply_to_label = self.t(i18n::REPONDRE_A);
                                let texture = media
                                    .as_ref()
                                    .filter(|m| m.kind == crate::message::MediaKind::Image)
                                    .and_then(|m| self.media_texture(ctx, &m.id));
                                // Bandeau façon Discord : liseré d'accent,
                                // « Répondre à » discret, nom en gras, extrait
                                // tronqué, croix collée à droite.
                                egui::Frame::default()
                                    .fill(crate::ui::theme::palette(ui).surface)
                                    .corner_radius(egui::CornerRadius::same(10))
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let (accent, _) = ui.allocate_exact_size(
                                                egui::vec2(3.0, 16.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                accent,
                                                2.0,
                                                crate::ui::theme::palette(ui).accent,
                                            );
                                            ui.label(
                                                egui::RichText::new(reply_to_label)
                                                    .small()
                                                    .color(crate::ui::theme::palette(ui).text_muted),
                                            );
                                            ui.label(
                                                egui::RichText::new(&author)
                                                    .small()
                                                    .color(crate::ui::theme::palette(ui).receipt_read)
                                                    .family(egui::FontFamily::Name(
                                                        super::BOLD_FAMILY.into(),
                                                    )),
                                            );
                                            if media.is_some() {
                                                super::media::render_reply_thumb(
                                                    ui,
                                                    texture.as_ref(),
                                                    20.0,
                                                );
                                            }
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if chip_remove_button(ui) {
                                                        self.replying_to = None;
                                                    }
                                                    ui.add_space(4.0);
                                                    ui.with_layout(
                                                        egui::Layout::left_to_right(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(&snippet)
                                                                        .small()
                                                                        .color(
                                                                            crate::ui::theme::palette(ui).text_muted,
                                                                        ),
                                                                )
                                                                .truncate(),
                                                            );
                                                        },
                                                    );
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(6.0);
                            }

                            if !self.composer.pending_attachments.is_empty() {
                                // Bandeau uniforme avec l'aperçu de réponse :
                                // même fond, même liseré d'accent, croix par
                                // pièce, boutons d'ajout, liste qui s'étend
                                // (défilement au-delà de quelques lignes).
                                let count = self.composer.pending_attachments.len();
                                let attachments_label =
                                    self.t(i18n::PIECES_JOINTES);
                                let add_files_btn_label = self.t(i18n::FICHIERS);
                                let add_folder_btn_label = self.t(i18n::DOSSIER);
                                egui::Frame::default()
                                    .fill(crate::ui::theme::palette(ui).surface)
                                    .corner_radius(egui::CornerRadius::same(10))
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let (accent, _) = ui.allocate_exact_size(
                                                egui::vec2(3.0, 16.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                accent,
                                                2.0,
                                                crate::ui::theme::palette(ui).accent,
                                            );
                                            ui.label(
                                                egui::RichText::new(attachments_label)
                                                    .small()
                                                    .color(crate::ui::theme::palette(ui).text_muted),
                                            );
                                            ui.label(
                                                egui::RichText::new(count.to_string())
                                                    .small()
                                                    .color(crate::ui::theme::palette(ui).receipt_read)
                                                    .family(egui::FontFamily::Name(
                                                        super::BOLD_FAMILY.into(),
                                                    )),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui.small_button(add_folder_btn_label).clicked()
                                                    {
                                                        picker_action = Some(
                                                            AttachmentMenuAction::AddFolder,
                                                        );
                                                    }
                                                    if ui.small_button(add_files_btn_label).clicked()
                                                    {
                                                        picker_action =
                                                            Some(AttachmentMenuAction::AddFiles);
                                                    }
                                                },
                                            );
                                        });
                                        ui.add_space(4.0);
                                        // Grille manuelle de chips à largeur
                                        // fixe : `horizontal_wrapped` ne
                                        // replie pas les conteneurs `Frame`
                                        // (largeur inconnue au placement), on
                                        // calcule donc nous-mêmes le nombre de
                                        // chips par ligne — ça ne déborde
                                        // jamais de la fenêtre.
                                        const CHIP_W: f32 = 200.0;
                                        const CHIP_GAP: f32 = 6.0;
                                        let per_row = ((ui.available_width() + CHIP_GAP)
                                            / (CHIP_W + CHIP_GAP))
                                            .floor()
                                            .max(1.0)
                                            as usize;
                                        let mut remove_index = None;
                                        egui::ScrollArea::vertical()
                                            .id_salt("attachments_scroll")
                                            .max_height(100.0)
                                            .show(ui, |ui| {
                                                ui.spacing_mut().item_spacing =
                                                    egui::vec2(CHIP_GAP, CHIP_GAP);
                                                let paths: Vec<_> = self
                                                    .composer
                                                    .pending_attachments
                                                    .iter()
                                                    .cloned()
                                                    .enumerate()
                                                    .collect();
                                                for line in paths.chunks(per_row) {
                                                    ui.horizontal(|ui| {
                                                        for (index, path) in line {
                                                            if attachment_chip(
                                                                ui,
                                                                path,
                                                                CHIP_W,
                                                            ) {
                                                                remove_index = Some(*index);
                                                            }
                                                        }
                                                    });
                                                }
                                            });
                                        if let Some(index) = remove_index {
                                            self.composer.pending_attachments.remove(index);
                                        }
                                    });
                                ui.add_space(6.0);
                            }

                            let menu_open_now =
                                emoji_shortcode_trigger(&self.composer.text, self.composer.cursor_char)
                                    .map(|(_, q)| !q.is_empty())
                                    .unwrap_or(false);

                            let available_w = ui.available_width();

                            let (resp, mut pressed_enter, changed, overflow_paste) =
                                composer::custom_composer_input(
                                    ui,
                                    &mut self.composer.text,
                                    &mut self.composer.cursor_char,
                                    &mut self.composer.has_focus,
                                    &mut self.composer.scroll_lines,
                                    &self.emoji.map,
                                    &self.emoji.textures,
                                    &self.emoji.alias_to_char,
                                    &self.emoji.aliases,
                                    menu_open_now,
                                    self.emoji.shortcode_selected,
                                    available_w,
                                    &mut self.composer.selection_anchor,
                                );

                            // Collage au-delà du plafond : le texte devient une
                            // pièce jointe .txt (le pipeline média streame sans
                            // limite) au lieu d'être tronqué ou perdu.
                            if let Some(text) = overflow_paste {
                                self.stash_overflow_paste(&text);
                            }

                            ui.add_space(3.0);
                            // Séparateur entre le champ de saisie et la barre
                            // d'actions : même teinte que le liseré du cadre.
                            let (sep_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 1.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().hline(
                                sep_rect.x_range(),
                                sep_rect.center().y,
                                egui::Stroke::new(1.0, crate::ui::theme::palette(ui).separator),
                            );
                            ui.add_space(3.0);

                            // Rangée du bas : pas de saisie de texte ici, juste
                            // l'indicateur de frappe (à gauche) et les boutons
                            // d'action (à droite).
                            let mut plus_btn_rect = egui::Rect::NOTHING;
                            ui.horizontal(|ui| {
                                ui.set_min_height(ACTION_BUTTON_SIZE[1]);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

                                        let send_btn = icon_button(
                                            ui,
                                            self.t(i18n::ENVOYER),
                                            false,
                                            paint_send_icon,
                                        );
                                        if send_btn.clicked() {
                                            pressed_enter = true;
                                        }

                                        let emoji_btn =
                                            if let Some(tex) = self.emoji.textures.get(ui.ctx(), 0) {
                                                icon_button(
                                                    ui,
                                                    self.t(i18n::BOUTON_EMOJIS),
                                                    self.show_emoji_picker,
                                                    |painter, rect, _| {
                                                        painter.image(
                                                            tex.id(),
                                                            rect,
                                                            egui::Rect::from_min_max(
                                                                egui::pos2(0.0, 0.0),
                                                                egui::pos2(1.0, 1.0),
                                                            ),
                                                            egui::Color32::WHITE,
                                                        );
                                                    },
                                                )
                                            } else {
                                                action_button(
                                                    ui,
                                                    egui::RichText::new("Em")
                                                        .size(16.0)
                                                        .color(
                                                            crate::ui::theme::palette(ui).text,
                                                        ),
                                                    self.t(i18n::BOUTON_EMOJIS),
                                                    self.show_emoji_picker,
                                                )
                                            };
                                        if emoji_btn.clicked() {
                                            self.show_emoji_picker = !self.show_emoji_picker;
                                            self.gif_picker.show = false;
                                            emoji_button_clicked = true;
                                        }

                                        let gif_btn = action_button(
                                            ui,
                                            egui::RichText::new("GIF")
                                                .size(10.5)
                                                .color(crate::ui::theme::palette(ui).text),
                                            gif_label,
                                            self.gif_picker.show,
                                        );
                                        if gif_btn.clicked() {
                                            if crate::config::klipy_api_key().is_some() {
                                                self.gif_picker.show = !self.gif_picker.show;
                                                self.show_emoji_picker = false;
                                                gif_button_clicked = true;
                                            } else {
                                                self.last_notification = Some(
                                                    self.t(i18n::CLE_API_KLIPY_MANQUANTE_ABCOM_KLIPY)
                                                    .to_string(),
                                                );
                                                self.notification_time = std::time::Instant::now();
                                            }
                                        }

                                        let plus_btn = icon_button(
                                            ui,
                                            self.t(i18n::AJOUTER_DES_FICHIERS_OU_DOSSIERS),
                                            self.show_attachment_menu,
                                            paint_plus_icon,
                                        );
                                        if plus_btn.clicked() {
                                            self.show_attachment_menu = !self.show_attachment_menu;
                                        }
                                        plus_btn_rect = plus_btn.rect;

                                        // Compteur de caractères, affiché à
                                        // l'approche du plafond (80 %), rouge
                                        // une fois la limite atteinte.
                                        let input_chars = self.composer.text.chars().count();
                                        if input_chars >= composer::MAX_INPUT_CHARS * 8 / 10 {
                                            let color =
                                                if input_chars >= composer::MAX_INPUT_CHARS {
                                                    crate::ui::theme::palette(ui).danger
                                                } else {
                                                    crate::ui::theme::palette(ui).text_muted
                                                };
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{input_chars} / {}",
                                                    composer::MAX_INPUT_CHARS
                                                ))
                                                .color(color)
                                                .small(),
                                            );
                                        }

                                        // Espace restant (à gauche) : indicateur
                                        // de frappe, vide si personne n'écrit.
                                        ui.with_layout(
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                if !typing_list.is_empty() {
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(format!(
                                                                "{} {}",
                                                                typing_list.join(", "),
                                                                self.t(i18n::EN_TRAIN_D_ECRIRE)
                                                            ))
                                                            .color(egui::Color32::WHITE)
                                                            .small(),
                                                        )
                                                        .truncate(),
                                                    );
                                                }
                                            },
                                        );
                                    },
                                );
                            });

                            if self.show_attachment_menu {
                                let (popup_action, popup_rect) = attachment_menu_popup(
                                    ctx,
                                    plus_btn_rect,
                                    add_files_label,
                                    add_folder_label,
                                );

                                if let Some(action) = popup_action {
                                    picker_action = Some(action);
                                    self.show_attachment_menu = false;
                                }

                                // Position inconnue (tactile, appui hors
                                // fenêtre) : on garde le menu ouvert plutôt que
                                // de le refermer sur un point (0, 0) supposé.
                                let pressed_at = ctx.input(|i| {
                                    i.pointer
                                        .any_pressed()
                                        .then(|| i.pointer.interact_pos())
                                        .flatten()
                                });
                                if let Some(pos) = pressed_at {
                                    if !plus_btn_rect.contains(pos) && !popup_rect.contains(pos) {
                                        self.show_attachment_menu = false;
                                    }
                                }
                            }

                            // Popup de suggestions shortcode
                            let shortcode_limit = match emoji_shortcode_trigger(
                                &self.composer.text,
                                self.composer.cursor_char,
                            ) {
                                Some((_, q)) if q.is_empty() => 5,
                                _ => 12,
                            };
                            let shortcode_list = super::emoji_picker::shortcode_suggestions(
                                &self.composer.text,
                                self.composer.cursor_char,
                                &self.emoji.alias_to_char,
                                &self.emoji.aliases,
                                shortcode_limit,
                            );

                            let mut clicked_shortcode: Option<String> = None;
                            if shortcode_list.is_empty() {
                                self.emoji.shortcode_selected = 0;
                            } else if self.emoji.shortcode_selected >= shortcode_list.len() {
                                self.emoji.shortcode_selected = shortcode_list.len() - 1;
                            }

                            // Consumir las flechas solo si el menú de shortcodes está abierto
                            if self.composer.has_focus && menu_open_now {
                                if ctx.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                                }) && !shortcode_list.is_empty()
                                {
                                    self.emoji.shortcode_selected = (self.emoji.shortcode_selected + 1)
                                        .min(shortcode_list.len() - 1);
                                }
                                if ctx.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                                }) && !shortcode_list.is_empty()
                                {
                                    self.emoji.shortcode_selected =
                                        self.emoji.shortcode_selected.saturating_sub(1);
                                }
                            }

                            if self.composer.has_focus && !shortcode_list.is_empty() {
                                super::emoji_picker::show_shortcode_popup(
                                    ctx,
                                    ui,
                                    &resp,
                                    &shortcode_list,
                                    &self.emoji.map,
                                    &self.emoji.textures,
                                    self.emoji.shortcode_selected,
                                    &mut clicked_shortcode,
                                );
                            }

                            if let Some(alias) = clicked_shortcode {
                                if let Some((start, _)) =
                                    emoji_shortcode_trigger(&self.composer.text, self.composer.cursor_char)
                                {
                                    if let Some(ch) = self.emoji.alias_to_char.get(&alias) {
                                        let end = self.composer.cursor_char;
                                        composer::replace_char_range(
                                            &mut self.composer.text,
                                            &mut self.composer.cursor_char,
                                            start,
                                            end,
                                            ch,
                                        );
                                        composer::sync_cursor(ctx, self.composer.cursor_char);
                                        self.composer.has_focus = true;
                                        self.show_emoji_picker = false;
                                    }
                                }
                            }

                            // Indicateur de frappe
                            if changed && self.last_typing_broadcast.elapsed().as_millis() > 1500
                            {
                                self.last_typing_broadcast = std::time::Instant::now();
                                let (my_name, targets) = {
                                    let s = self.state.lock_safe();
                                    (s.my_username.clone(), s.selected_transfer_targets())
                                };
                                for target in targets {
                                    self.net.try_send_best_effort(TypingRequest {
                                        to_peer: target.username,
                                        to_addr: target.addr,
                                        indicator: TypingIndicator {
                                            from: my_name.clone(),
                                        },
                                    });
                                }
                            }

                            // L'envoi clavier passe par Cmd+Entrée (macOS) ou
                            // Ctrl+Entrée ; Entrée seule insère une nouvelle
                            // ligne dans le composeur.
                            let pressed_enter_fallback = ui.input(|i| {
                                i.key_pressed(egui::Key::Enter)
                                    && (i.modifiers.command || i.modifiers.ctrl)
                            });

                            if should_send_message(
                                pressed_enter,
                                pressed_enter_fallback,
                                menu_open_now,
                                &self.composer.text,
                            ) && send_current_message(self)
                            {
                                self.composer.selection_anchor = None;
                                resp.request_focus();
                                self.show_emoji_picker = false;
                            }
                        });
                    });
            });

        // Defer the file/folder picker to the next frame so it runs before egui
        // rendering, avoiding an AppKit run-loop conflict on macOS.
        match picker_action {
            Some(AttachmentMenuAction::AddFiles) => {
                self.pending_picker = 1;
                ctx.request_repaint();
            }
            Some(AttachmentMenuAction::AddFolder) => {
                self.pending_picker = 2;
                ctx.request_repaint();
            }
            None => {}
        }

        (emoji_button_clicked, gif_button_clicked)
    }
}

#[cfg(test)]
#[path = "../../tests/test_ui_input_bar.rs"]
mod tests;
