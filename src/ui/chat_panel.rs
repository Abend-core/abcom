use chrono::{Datelike, Local, NaiveDate, TimeZone};
use eframe::egui;

use crate::message::{ChatMessage, ReactionEntry};

use super::{AbcomApp, ReplyTarget, UiLanguage};

/// Diamètre de l'avatar affiché en tête de chaque groupe de messages.
const AVATAR_SIZE: f32 = 40.0;
/// Espace horizontal entre l'avatar et le texte du message.
const AVATAR_GUTTER: f32 = 12.0;
/// Espace vertical séparant deux groupes de messages.
const GROUP_SPACING: f32 = 10.0;
/// Écart de temps au-delà duquel un nouvel en-tête est ouvert pour un même
/// auteur (façon Discord/Cinny). Évite que des messages espacés de plusieurs
/// heures ou jours paraissent envoyés d'un coup. Ajustable.
const GROUP_BREAK_SECS: u64 = 5 * 60;
/// Taille (carrée) d'un bouton de la barre d'actions au survol.
const HOVER_BTN_SIZE: f32 = 26.0;
/// Taille d'une texture d'emoji peinte dans un bouton de la barre de survol
/// ou d'une pastille de réaction.
const HOVER_EMOJI_SIZE: f32 = 16.0;
/// Couleur du nom pour nos propres messages (conservée partout).
pub(crate) const OWN_NAME_COLOR: egui::Color32 = egui::Color32::from_rgb(80, 200, 120);
/// Couleur du nom d'un autre pair en conversation 1-à-1.
pub(crate) const PEER_NAME_COLOR: egui::Color32 = egui::Color32::from_rgb(100, 180, 255);

/// Palette de couleurs distinctes pour les pairs dans les vues multi-personnes
/// (groupes et « Tous »), inspirée des couleurs d'utilisateur de Cinny.
const PEER_PALETTE: [egui::Color32; 8] = [
    egui::Color32::from_rgb(128, 195, 255),
    egui::Color32::from_rgb(255, 153, 253),
    egui::Color32::from_rgb(102, 255, 212),
    egui::Color32::from_rgb(255, 128, 164),
    egui::Color32::from_rgb(255, 163, 102),
    egui::Color32::from_rgb(51, 252, 255),
    egui::Color32::from_rgb(158, 153, 255),
    egui::Color32::from_rgb(197, 255, 153),
];

/// Couleur déterministe attribuée à un pair (même nom → même couleur) pour le
/// distinguer des autres participants dans une conversation multi-personnes.
pub(crate) fn peer_color(username: &str) -> egui::Color32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    username.hash(&mut hasher);
    PEER_PALETTE[(hasher.finish() as usize) % PEER_PALETTE.len()]
}

const MONTHS_FR: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Jour local d'un message à partir de son instant Unix (`None` si absent).
pub(crate) fn message_day(msg: &ChatMessage) -> Option<NaiveDate> {
    let epoch = msg.timestamp_epoch?;
    Local
        .timestamp_opt(epoch as i64, 0)
        .single()
        .map(|dt| dt.date_naive())
}

/// Heure d'en-tête au format 24 h, dérivée de l'instant Unix si présent,
/// sinon repli sur la chaîne `timestamp` (anciens messages / pairs).
pub(crate) fn header_time(msg: &ChatMessage) -> String {
    match msg
        .timestamp_epoch
        .and_then(|e| Local.timestamp_opt(e as i64, 0).single())
    {
        Some(dt) => dt.format("%H:%M").to_string(),
        None => msg.timestamp.clone(),
    }
}

/// Décide si un message ouvre un nouveau groupe (nouvel en-tête avec avatar).
/// Vrai si l'auteur change, si le jour change, ou si l'écart de temps dépasse
/// `GROUP_BREAK_SECS`. Sans instant comparable, on se rabat sur l'auteur seul.
pub(crate) fn starts_new_group(
    prev_from: Option<&str>,
    prev_epoch: Option<u64>,
    from: &str,
    epoch: Option<u64>,
    day_changed: bool,
) -> bool {
    if prev_from != Some(from) || day_changed {
        return true;
    }
    match (prev_epoch, epoch) {
        (Some(prev), Some(now)) => now.saturating_sub(prev) > GROUP_BREAK_SECS,
        _ => false,
    }
}

/// Libellé localisé d'un séparateur de date (« Aujourd'hui », « Hier » ou date
/// complète selon la langue).
pub(crate) fn day_divider_label(date: NaiveDate, today: NaiveDate, language: UiLanguage) -> String {
    if date == today {
        return match language {
            UiLanguage::French => "Aujourd'hui".to_string(),
            UiLanguage::English => "Today".to_string(),
        };
    }
    if Some(date) == today.pred_opt() {
        return match language {
            UiLanguage::French => "Hier".to_string(),
            UiLanguage::English => "Yesterday".to_string(),
        };
    }
    let (day, month, year) = (date.day(), date.month0() as usize, date.year());
    match language {
        UiLanguage::French => format!("{} {} {}", day, MONTHS_FR[month], year),
        UiLanguage::English => format!("{} {}, {}", MONTHS_EN[month], day, year),
    }
}

/// Dessine un séparateur de date pleine largeur : une ligne fine traversée par
/// le libellé centré, façon Discord/Cinny.
fn render_day_divider(ui: &mut egui::Ui, label: &str) {
    ui.add_space(14.0);
    let line_color = egui::Color32::from_gray(80);
    let text_color = egui::Color32::from_gray(150);
    let font = egui::TextStyle::Small.resolve(ui.style());

    let full_width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(full_width, 18.0), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(label.to_string(), font, text_color);
    let text_w = galley.size().x;
    let gap = 12.0;
    let center_y = rect.center().y;
    let mid_x = rect.center().x;

    let left_end = mid_x - text_w / 2.0 - gap;
    let right_start = mid_x + text_w / 2.0 + gap;
    let stroke = egui::Stroke::new(1.0, line_color);
    painter.line_segment(
        [
            egui::pos2(rect.left(), center_y),
            egui::pos2(left_end, center_y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(right_start, center_y),
            egui::pos2(rect.right(), center_y),
        ],
        stroke,
    );
    painter.galley(
        egui::pos2(mid_x - text_w / 2.0, center_y - galley.size().y / 2.0),
        galley,
        text_color,
    );
    ui.add_space(6.0);
}

/// En-tête d'un groupe de messages : nom coloré suivi, collé à droite, de
/// l'heure d'envoi (format 24 h) et, pour nos messages, de l'accusé de lecture.
fn render_message_header(
    ui: &mut egui::Ui,
    display_name: &str,
    timestamp: &str,
    name_color: egui::Color32,
    receipt: Option<(bool, bool)>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            egui::RichText::new(display_name)
                .color(name_color)
                .family(egui::FontFamily::Name(super::BOLD_FAMILY.into())),
        );
        ui.label(
            egui::RichText::new(timestamp)
                .small()
                .color(egui::Color32::from_gray(140)),
        );
        if let Some((delivered, read)) = receipt {
            show_receipt(ui, delivered, read);
        }
    });
}

/// Rend le corps d'un message (texte Markdown puis média éventuel) et renvoie
/// l'action déclenchée sur le média, le cas échéant.
#[allow(clippy::too_many_arguments)]
fn render_message_body(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    parsed: &super::markdown::ParsedMarkdown,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
    media_textures: &std::collections::HashMap<String, Option<egui::TextureHandle>>,
    media_progress: &std::collections::HashMap<String, crate::message::MediaProgress>,
) -> Option<super::media::MediaAction> {
    if !msg.content.is_empty() {
        super::markdown::render_parsed_markdown(ui, parsed, emoji_map, emoji_textures);
    }
    if let Some(media) = &msg.media {
        // Pendant le transfert : barre de progression au lieu de la carte.
        if let Some(progress) = media_progress.get(&media.id) {
            super::media::render_media_progress(ui, media, progress);
            return None;
        }
        let texture = media_textures.get(&media.id).and_then(|t| t.as_ref());
        return super::media::render_media_block(ui, media, texture);
    }
    None
}

/// Enregistre l'action média choisie (ouverture ou téléchargement) dans les
/// variables collectées pendant le rendu, traitées après la zone défilante.
fn apply_media_action(
    action: super::media::MediaAction,
    msg: &ChatMessage,
    view_open: &mut Option<String>,
    download: &mut Option<(String, String)>,
) {
    let Some(media) = &msg.media else { return };
    match action {
        super::media::MediaAction::View => *view_open = Some(media.id.clone()),
        super::media::MediaAction::Download => {
            *download = Some((media.id.clone(), media.filename.clone()))
        }
    }
}

/// Peint la texture PNG d'un emoji dans `rect` (jamais un glyphe police, cf.
/// `emoji_picker::render_inline`), ou ne peint rien si la texture est absente
/// du registre (emoji inconnu/non chargé).
fn paint_emoji_texture(
    ui: &egui::Ui,
    rect: egui::Rect,
    emoji: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
) {
    if let Some((_, texture)) = emoji_map
        .get(emoji)
        .and_then(|&idx| emoji_textures.get(idx))
    {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}

/// Rend les pastilles de réaction sous un message (emoji + compteur),
/// surlignées si l'utilisateur courant a réagi. Renvoie l'emoji cliqué ;
/// l'appelant décide d'ajouter ou de retirer selon l'état actuel (toggle).
fn render_reaction_pills(
    ui: &mut egui::Ui,
    reactions: &[ReactionEntry],
    my_username: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
) -> Option<String> {
    if reactions.is_empty() {
        return None;
    }
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        for entry in reactions {
            let mine = entry.users.iter().any(|u| u == my_username);
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(46.0, 24.0), egui::Sense::click());
            if !ui.is_rect_visible(rect) {
                continue;
            }
            let fill = if mine {
                egui::Color32::from_rgb(70, 90, 140)
            } else if resp.hovered() {
                ui.visuals().widgets.hovered.bg_fill
            } else {
                egui::Color32::from_rgb(50, 52, 58)
            };
            let stroke = if mine {
                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 150, 255))
            } else {
                egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 72, 78))
            };
            ui.painter()
                .rect(rect, 6.0, fill, stroke, egui::StrokeKind::Inside);
            let emoji_rect = egui::Rect::from_center_size(
                rect.left_center() + egui::vec2(14.0, 0.0),
                egui::vec2(HOVER_EMOJI_SIZE, HOVER_EMOJI_SIZE),
            );
            paint_emoji_texture(ui, emoji_rect, &entry.emoji, emoji_map, emoji_textures);
            ui.painter().text(
                rect.right_center() - egui::vec2(8.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                entry.users.len().to_string(),
                egui::TextStyle::Small.resolve(ui.style()),
                ui.visuals().text_color(),
            );
            let tooltip = entry.users.join(", ");
            if resp.on_hover_text(tooltip).clicked() {
                clicked = Some(entry.emoji.clone());
            }
        }
    });
    clicked
}

/// Taille du mini-avatar affiché dans une citation de réponse.
const REPLY_QUOTE_AVATAR: f32 = 16.0;
/// Durée du flash de surlignage après un saut vers un message (secondes).
const HIGHLIGHT_SECS: f32 = 2.0;

/// Citation compacte au-dessus d'un message qui répond à un autre (façon
/// Discord) : ligne de liaison qui part de l'avatar, mini-avatar et nom
/// coloré de l'auteur d'origine, extrait sur une seule ligne. Cliquable pour
/// remonter au message d'origine (renvoie `true` au clic). `resolved` est
/// `None` si le message d'origine a expiré du ring-buffer ou n'a jamais été
/// reçu par ce pair.
#[allow(clippy::too_many_arguments)]
fn render_reply_quote(
    ui: &mut egui::Ui,
    msg_hash: u64,
    resolved: Option<&ChatMessage>,
    author_name: &str,
    author_color: egui::Color32,
    author_avatar: Option<&egui::TextureHandle>,
    media_texture: Option<&egui::TextureHandle>,
    not_found_label: &str,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let left = ui.cursor().left();
        let spine_x = left + AVATAR_SIZE / 2.0;
        ui.add_space(AVATAR_SIZE + AVATAR_GUTTER);

        // Fond arrondi inséré sous le contenu, rempli seulement au survol.
        let hover_bg = ui.painter().add(egui::Shape::Noop);
        let rect = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                match resolved {
                    Some(orig) => {
                        super::avatar::show_avatar(
                            ui,
                            author_avatar,
                            author_name,
                            REPLY_QUOTE_AVATAR,
                        );
                        ui.label(
                            egui::RichText::new(author_name)
                                .small()
                                .color(author_color)
                                .family(egui::FontFamily::Name(super::BOLD_FAMILY.into())),
                        );
                        if orig.media.is_some() {
                            super::media::render_reply_thumb(
                                ui,
                                media_texture,
                                REPLY_QUOTE_AVATAR,
                            );
                        }
                        let snippet = if orig.content.is_empty() && orig.media.is_some() {
                            "📎".to_string()
                        } else {
                            super::media::elide(&orig.content, 90)
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(snippet)
                                    .small()
                                    .color(egui::Color32::from_gray(165)),
                            )
                            .truncate(),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new(not_found_label)
                                .small()
                                .italics()
                                .color(egui::Color32::from_gray(120)),
                        );
                    }
                }
            })
            .response
            .rect;

        // Ligne de liaison : descend du coin arrondi vers l'avatar du message
        // qui répond, et rejoint horizontalement la citation.
        let stroke = egui::Stroke::new(2.0, egui::Color32::from_gray(90));
        let cy = rect.center().y;
        let corner = 8.0;
        let painter = ui.painter();
        painter.line_segment(
            [
                egui::pos2(spine_x + corner, cy),
                egui::pos2(rect.left() - 6.0, cy),
            ],
            stroke,
        );
        painter.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
            [
                egui::pos2(spine_x, cy + corner),
                egui::pos2(spine_x, cy),
                egui::pos2(spine_x + corner, cy),
            ],
            false,
            egui::Color32::TRANSPARENT,
            stroke,
        ));
        painter.line_segment(
            [
                egui::pos2(spine_x, cy + corner),
                egui::pos2(spine_x, rect.bottom() + 4.0),
            ],
            stroke,
        );

        if resolved.is_some() {
            let hit = rect.expand2(egui::vec2(4.0, 2.0));
            let resp = ui.interact(
                hit,
                egui::Id::new(("reply_quote", msg_hash)),
                egui::Sense::click(),
            );
            if resp.hovered() {
                ui.painter().set(
                    hover_bg,
                    egui::Shape::rect_filled(
                        hit,
                        4.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10),
                    ),
                );
            }
            clicked = resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked();
        }
    });
    ui.add_space(2.0);
    clicked
}

/// Résultat d'un rendu de la barre d'actions flottante au survol.
struct HoverToolbarResult {
    /// Le pointeur est actuellement au-dessus de la barre elle-même (pour
    /// éviter que le survol ne « clignote » en passant de la ligne à la
    /// barre flottante qui la recouvre partiellement).
    pointer_over_toolbar: bool,
    reply_clicked: bool,
    quick_emoji: Option<String>,
}

impl AbcomApp {
    /// Barre d'actions flottante affichée au survol d'un message : emojis
    /// récents, "+" (picker complet de réaction) et "répondre". Pas de
    /// bouton de transfert. `row_rect` est le rectangle pleine largeur de la
    /// ligne : la barre est collée au bord droit du fil (position stable
    /// quel que soit le message) et chevauche le haut de la ligne, façon
    /// Discord.
    fn show_hover_toolbar(
        &mut self,
        ctx: &egui::Context,
        msg_hash: u64,
        row_rect: egui::Rect,
        reply_label: &str,
        add_reaction_label: &str,
    ) -> HoverToolbarResult {
        // Lookup indexé via `emoji_map` (pas de recherche linéaire dans les
        // 323 textures à chaque frame de survol).
        let textures: Vec<(String, egui::TextureHandle)> = self
            .recent_reaction_emojis
            .iter()
            .filter_map(|e| {
                self.emoji_map
                    .get(e)
                    .and_then(|&idx| self.emoji_textures.get(idx))
                    .cloned()
            })
            .collect();

        let toolbar_w = HOVER_BTN_SIZE * (textures.len() as f32 + 2.0) + 6.0;
        let toolbar_h = HOVER_BTN_SIZE + 10.0;
        let anchor = egui::pos2(
            row_rect.right() - toolbar_w - 12.0,
            row_rect.top() - toolbar_h * 0.5,
        );

        let mut quick_emoji = None;
        let mut reply_clicked = false;
        let mut plus_rect = None;

        let area = egui::Area::new(egui::Id::new(("msg_hover_toolbar", msg_hash)))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor);
        let resp = area.show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (ch, _) in &textures {
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(HOVER_BTN_SIZE, HOVER_BTN_SIZE),
                                egui::Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    6.0,
                                    ui.visuals().widgets.hovered.bg_fill,
                                );
                            }
                            let emoji_rect = egui::Rect::from_center_size(
                                rect.center(),
                                egui::vec2(HOVER_EMOJI_SIZE, HOVER_EMOJI_SIZE),
                            );
                            paint_emoji_texture(
                                ui,
                                emoji_rect,
                                ch,
                                &self.emoji_map,
                                &self.emoji_textures,
                            );
                            if resp.clicked() {
                                quick_emoji = Some(ch.clone());
                            }
                        }

                        let (plus_r, plus_resp) = ui.allocate_exact_size(
                            egui::vec2(HOVER_BTN_SIZE, HOVER_BTN_SIZE),
                            egui::Sense::click(),
                        );
                        if plus_resp.hovered() {
                            ui.painter().rect_filled(
                                plus_r,
                                6.0,
                                ui.visuals().widgets.hovered.bg_fill,
                            );
                        }
                        ui.painter().text(
                            plus_r.center(),
                            egui::Align2::CENTER_CENTER,
                            "+",
                            egui::FontId::proportional(16.0),
                            ui.visuals().text_color(),
                        );
                        if plus_resp.on_hover_text(add_reaction_label).clicked() {
                            plus_rect = Some(plus_r);
                        }

                        let (reply_r, reply_resp) = ui.allocate_exact_size(
                            egui::vec2(HOVER_BTN_SIZE, HOVER_BTN_SIZE),
                            egui::Sense::click(),
                        );
                        if reply_resp.hovered() {
                            ui.painter().rect_filled(
                                reply_r,
                                6.0,
                                ui.visuals().widgets.hovered.bg_fill,
                            );
                        }
                        ui.painter().text(
                            reply_r.center(),
                            egui::Align2::CENTER_CENTER,
                            "↩",
                            egui::FontId::proportional(16.0),
                            ui.visuals().text_color(),
                        );
                        if reply_resp.on_hover_text(reply_label).clicked() {
                            reply_clicked = true;
                        }
                    });
                });
        });

        if let Some(rect) = plus_rect {
            self.reaction_picker_open = Some((msg_hash, rect));
        }

        let pointer_over_toolbar = ctx
            .input(|i| i.pointer.interact_pos())
            .map(|p| resp.response.rect.contains(p))
            .unwrap_or(false);

        HoverToolbarResult {
            pointer_over_toolbar,
            reply_clicked,
            quick_emoji,
        }
    }
}

impl AbcomApp {
    /// Zone centrale : fil de la conversation sélectionnée. Le rendu
    /// consomme exclusivement le cache dérivé (`ui/snapshot.rs`) : aucun
    /// verrou sur `AppState`, aucun clone de conversation, aucun re-parse
    /// markdown par frame. Le fil est fenêtré façon Discord : seuls les
    /// derniers messages sont rendus, remonter charge les 100 précédents.
    pub(crate) fn show_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let rows = self.chat_cache.rows.clone();
            let my_name = self.chat_cache.my_name.clone();
            let selected_conv: Option<String> =
                self.chat_cache.conversation().map(str::to_string);
            let private_peer = selected_conv
                .as_deref()
                .filter(|c| !c.starts_with('#'))
                .map(str::to_string);
            let is_broadcast = selected_conv.is_none();

            let conversation_title = match &self.chat_cache.private_peer_display {
                Some(name) => name.clone(),
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

            // Popup participants (instantané depuis le cache latéral).
            if self.show_participants {
                let conv_name = self
                    .sidebar_cache
                    .selected_conversation
                    .clone()
                    .unwrap_or_else(|| self.tr("Tous", "All").to_string());
                let my_name2 = self.sidebar_cache.my_username.clone();
                let sel_conv = self.sidebar_cache.selected_conversation.clone();
                let peers = self.sidebar_cache.peers.clone();
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
                            for peer in peers.iter() {
                                ui.label(&peer.username);
                            }
                            if peers.is_empty() {
                                ui.label(
                                    self.tr(
                                        "Aucun participant connecté",
                                        "No connected participant",
                                    ),
                                );
                            }
                        } else {
                            ui.label(format!("{} ({})", my_name2, self.tr("vous", "you")));
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

            let not_found_label = self.tr(
                "Message d'origine introuvable",
                "Original message not found",
            );
            let reply_label = self.tr("Répondre", "Reply");
            let add_reaction_label = self.tr("Ajouter une réaction", "Add reaction");

            // Aire de messages
            let scroll_out = egui::ScrollArea::vertical()
                .id_salt("chat_scroll")
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if rows.is_empty() {
                        ui.add_space(50.0);
                        ui.label(
                            egui::RichText::new(self.tr("Aucun message", "No message")).weak(),
                        );
                    }

                    for (i, row) in rows[start..].iter().enumerate() {
                        let msg = &row.msg;
                        let hash = row.hash;

                        // Première ligne d'une fenêtre tronquée : séparateur
                        // de date forcé (situe la coupure) et en-tête forcé
                        // (pas de continuation orpheline sans avatar).
                        let window_head = i == 0 && start > 0;
                        let divider = row
                            .day_divider
                            .as_ref()
                            .or(if window_head { row.day_label.as_ref() } else { None });
                        if let Some(label) = divider {
                            render_day_divider(ui, label);
                        }
                        let starts_group = row.starts_group || window_head;

                        let mut reaction_clicked: Option<String> = None;
                        let mut reply_quote_clicked = false;

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

                        let row_resp = if starts_group {
                            ui.add_space(GROUP_SPACING);
                            ui.vertical(|ui| {
                                if let Some(reply) = &row.reply {
                                    if render_reply_quote(
                                        ui,
                                        hash,
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
                                        );
                                        if let Some(action) = render_message_body(
                                            ui,
                                            msg,
                                            &row.markdown,
                                            &self.emoji_map,
                                            &self.emoji_textures,
                                            &media_textures,
                                            &self.media_progress,
                                        ) {
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
                                            &self.emoji_map,
                                            &self.emoji_textures,
                                        ) {
                                            reaction_clicked = Some(emoji);
                                        }
                                    });
                                });
                            })
                        } else {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let (gutter_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(AVATAR_SIZE + AVATAR_GUTTER, 20.0),
                                    egui::Sense::hover(),
                                );
                                if ui.rect_contains_pointer(gutter_rect) {
                                    ui.painter().text(
                                        gutter_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &row.header_time,
                                        egui::TextStyle::Small.resolve(ui.style()),
                                        egui::Color32::from_gray(140),
                                    );
                                }
                                ui.vertical(|ui| {
                                    if let Some(action) = render_message_body(
                                        ui,
                                        msg,
                                        &row.markdown,
                                        &self.emoji_map,
                                        &self.emoji_textures,
                                        &media_textures,
                                        &self.media_progress,
                                    ) {
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
                                        &self.emoji_map,
                                        &self.emoji_textures,
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
                        let row_hovered = ui.rect_contains_pointer(row_rect);
                        if row_hovered {
                            self.hover_toolbar_target = Some(hash);
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
                        } else if self.hover_toolbar_target == Some(hash) {
                            let tint = if ui.visuals().dark_mode {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 7)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 10)
                            };
                            ui.painter().set(
                                row_bg,
                                egui::Shape::rect_filled(row_rect, 0.0, tint),
                            );
                        }

                        let mut reply_requested = false;
                        if self.hover_toolbar_target == Some(hash) {
                            let result = self.show_hover_toolbar(
                                ctx,
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

                    // Offres de médias volumineux (> 1 Go) à accepter/refuser.
                    self.render_media_offers(ui);
                });

            // Pagination façon Discord : arrivé près du haut, charger les 100
            // messages précédents — d'abord depuis la fenêtre mémoire, puis
            // depuis SQLite quand elle est épuisée — et compenser l'offset de
            // la hauteur ajoutée (aucun saut visuel, pas de bouton).
            if let Some(prev_height) = self.chat_prepend_fix {
                let delta = scroll_out.content_size.y - prev_height;
                if delta > 0.0 {
                    // Le contenu ajouté est arrivé : compenser l'offset.
                    self.chat_prepend_fix = None;
                    let mut state = scroll_out.state.clone();
                    state.offset.y += delta;
                    state.store(ctx, scroll_out.id);
                    ctx.request_repaint();
                }
                // delta == 0 : requête SQLite encore en vol, on attend.
            } else if scroll_out.state.offset.y < 400.0 && !rows.is_empty() {
                if start > 0 {
                    self.chat_visible_count =
                        (self.chat_visible_count + super::CHAT_WINDOW_STEP).min(total);
                    self.chat_prepend_fix = Some(scroll_out.content_size.y);
                    ctx.request_repaint();
                } else if !self.loading_older
                    && self.state.lock().unwrap().request_older_messages()
                {
                    self.loading_older = true;
                    self.chat_prepend_fix = Some(scroll_out.content_size.y);
                }
            }

            // Application des actions médias collectées pendant le rendu.
            if let Some(id) = media_view_open {
                self.media_viewer = Some(id);
            }
            if let Some((id, filename)) = media_download {
                self.download_media(&id, &filename);
            }
        });
    }

    /// Bandeaux d'acceptation des médias volumineux (> 1 Go) reçus. Accepter →
    /// le pair streame alors le média ; Refuser → l'envoi est abandonné.
    fn render_media_offers(&mut self, ui: &mut egui::Ui) {
        if self.pending_media_offers.is_empty() {
            return;
        }
        let mut decided: Option<(usize, bool)> = None;

        for (index, offer) in self.pending_media_offers.iter().enumerate() {
            ui.add_space(6.0);
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(48, 52, 60))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            offer.from,
                            self.tr(
                                "souhaite vous envoyer un fichier",
                                "wants to send you a file"
                            )
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
                        if ui.button(self.tr("Refuser", "Decline")).clicked() {
                            decided = Some((index, false));
                        }
                        if ui.button(self.tr("Accepter", "Accept")).clicked() {
                            decided = Some((index, true));
                        }
                    });
                });
        }

        if let Some((index, accept)) = decided {
            let offer = self.pending_media_offers.remove(index);
            if !accept {
                // Refus : annoter le fil (message attribué à l'expéditeur).
                let mut s = self.state.lock().unwrap();
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
fn show_receipt(ui: &mut egui::Ui, delivered: bool, read: bool) {
    let double = delivered || read;
    let w = if double { 17.0_f32 } else { 9.0_f32 };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 12.0), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let color = if read {
        egui::Color32::from_rgb(80, 180, 255) // bleu = lu
    } else {
        egui::Color32::from_gray(160) // gris = envoyé ou livré
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
#[path = "../tests/test_ui_chat_panel.rs"]
mod tests;
