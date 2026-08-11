//! Rendu d'une ligne du fil : en-tête, corps, réactions, citations, accusés.

use chrono::{Datelike, Local, NaiveDate, TimeZone};
use eframe::egui;

use crate::message::{ChatMessage, ReactionEntry};

use super::show_receipt;
use crate::ui::i18n;
use crate::ui::UiLanguage;

/// Diamètre de l'avatar affiché en tête de chaque groupe de messages.
pub(super) const AVATAR_SIZE: f32 = 40.0;
/// Espace horizontal entre l'avatar et le texte du message.
pub(super) const AVATAR_GUTTER: f32 = 12.0;
/// Espace vertical séparant deux groupes de messages.
pub(super) const GROUP_SPACING: f32 = 10.0;
/// Petite marge à droite du fil : évite que les messages (texte, fond de
/// survol, tableaux) ne collent au bord, pour la même respiration que sous le
/// dernier message avant la barre de saisie.
pub(super) const MESSAGE_RIGHT_MARGIN: f32 = 8.0;
/// Écart de temps au-delà duquel un nouvel en-tête est ouvert pour un même
/// auteur (façon Discord/Cinny). Évite que des messages espacés de plusieurs
/// heures ou jours paraissent envoyés d'un coup. Ajustable.
pub(crate) const GROUP_BREAK_SECS: u64 = 5 * 60;
/// Taille (carrée) d'un bouton de la barre d'actions au survol.
pub(super) const HOVER_BTN_SIZE: f32 = 26.0;
/// Taille d'une texture d'emoji peinte dans un bouton de la barre de survol
/// ou d'une pastille de réaction.
pub(super) const HOVER_EMOJI_SIZE: f32 = 16.0;

/// Palette de couleurs distinctes pour les pairs dans les vues multi-personnes.
/// Les teintes vives conviennent au fond sombre ; leurs équivalentes assombries
/// (`PEER_PALETTE_LIGHT`) restent lisibles sur fond clair.
const PEER_PALETTE_DARK: [egui::Color32; 8] = [
    egui::Color32::from_rgb(128, 195, 255),
    egui::Color32::from_rgb(255, 153, 253),
    egui::Color32::from_rgb(102, 255, 212),
    egui::Color32::from_rgb(255, 128, 164),
    egui::Color32::from_rgb(255, 163, 102),
    egui::Color32::from_rgb(51, 252, 255),
    egui::Color32::from_rgb(158, 153, 255),
    egui::Color32::from_rgb(197, 255, 153),
];

/// Mêmes teintes, assombries pour rester lisibles sur un fond clair.
const PEER_PALETTE_LIGHT: [egui::Color32; 8] = [
    egui::Color32::from_rgb(21, 94, 156),
    egui::Color32::from_rgb(158, 30, 148),
    egui::Color32::from_rgb(12, 122, 96),
    egui::Color32::from_rgb(163, 30, 66),
    egui::Color32::from_rgb(158, 82, 15),
    egui::Color32::from_rgb(12, 116, 128),
    egui::Color32::from_rgb(76, 62, 178),
    egui::Color32::from_rgb(86, 118, 24),
];

/// Couleur déterministe attribuée à un pair (même nom → même couleur) pour le
/// distinguer des autres participants dans une conversation multi-personnes.
pub(crate) fn peer_color_for(username: &str, dark_mode: bool) -> egui::Color32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    username.hash(&mut hasher);
    let palette = if dark_mode {
        PEER_PALETTE_DARK
    } else {
        PEER_PALETTE_LIGHT
    };
    palette[(hasher.finish() as usize) % palette.len()]
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
        return crate::ui::i18n::AUJOURD_HUI.get(language).to_string();
    }
    if Some(date) == today.pred_opt() {
        return crate::ui::i18n::HIER.get(language).to_string();
    }
    let (day, month, year) = (date.day(), date.month0() as usize, date.year());
    match language {
        UiLanguage::French => format!("{} {} {}", day, MONTHS_FR[month], year),
        UiLanguage::English => format!("{} {}, {}", MONTHS_EN[month], day, year),
    }
}

/// Dessine un séparateur de date pleine largeur : une ligne fine traversée par
/// le libellé centré, façon Discord/Cinny.
pub(super) fn render_day_divider(ui: &mut egui::Ui, label: &str) {
    ui.add_space(14.0);
    let line_color = crate::ui::theme::palette(ui).separator;
    let text_color = crate::ui::theme::palette(ui).text_muted;
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
/// l'heure d'envoi (format 24 h) et de l'accusé de lecture — coches pour nos
/// messages en 1-à-1, bouton « … » (liste nominative reçu/lu) en salon/« Tous ».
#[allow(clippy::too_many_arguments)]
pub(super) fn render_message_header(
    ui: &mut egui::Ui,
    display_name: &str,
    timestamp: &str,
    name_color: egui::Color32,
    receipt: Option<(bool, bool, bool)>,
    receipt_detail: Option<&crate::app::ReceiptDetail>,
    row_hash: u64,
    language: UiLanguage,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            egui::RichText::new(display_name)
                .color(name_color)
                .family(egui::FontFamily::Name(crate::ui::BOLD_FAMILY.into())),
        );
        ui.label(
            egui::RichText::new(timestamp)
                .small()
                .color(crate::ui::theme::palette(ui).text_muted),
        );
        if let Some(detail) = receipt_detail {
            show_receipt_detail_button(ui, detail, row_hash, language);
        } else if let Some((delivered, read, failed)) = receipt {
            show_receipt(ui, delivered, read, failed);
        }
    });
}

/// Œil peint : un « ... » ne disait pas de quoi il était le détail, et le
/// glyphe 👁 n'est pas rendu de façon fiable par les polices embarquées.
fn paint_eye_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    const STEPS: usize = 10;
    let stroke = egui::Stroke::new(1.2, color);
    let center = rect.center();
    let lid = |sign: f32| {
        (0..=STEPS)
            .map(|i| {
                let t = i as f32 / STEPS as f32;
                egui::pos2(
                    rect.left() + t * rect.width(),
                    center.y + sign * rect.height() * 0.5 * (std::f32::consts::PI * t).sin(),
                )
            })
            .collect::<Vec<_>>()
    };
    painter.add(egui::Shape::line(lid(-1.0), stroke));
    painter.add(egui::Shape::line(lid(1.0), stroke));
    painter.circle_filled(center, rect.height() * 0.28, color);
}

/// Compteur de lecture des salons et de « Tous » : œil suivi de « lu / total ».
/// Un clic ouvre le détail nominatif de qui a reçu et qui a lu.
///
/// Les coches du 1-à-1 n'ont pas de sens à plusieurs — chaque membre peut avoir
/// reçu ou lu indépendamment —, d'où un compteur plutôt qu'un état unique.
pub(super) fn show_receipt_detail_button(
    ui: &mut egui::Ui,
    detail: &crate::app::ReceiptDetail,
    row_hash: u64,
    language: UiLanguage,
) {
    let popup_id = ui.make_persistent_id(("receipt_popup", row_hash));
    let read = detail.read_by.len();
    // Tout le monde a lu : même bleu que la double coche du 1-à-1.
    let color = if read > 0 && read >= detail.audience {
        crate::ui::theme::palette(ui).receipt_read
    } else {
        crate::ui::theme::palette(ui).text_muted
    };

    let count = format!("{read}/{}", detail.audience);
    let galley = ui.painter().layout_no_wrap(
        count.clone(),
        egui::TextStyle::Small.resolve(ui.style()),
        color,
    );
    const EYE: egui::Vec2 = egui::vec2(14.0, 9.0);
    let (rect, btn) = ui.allocate_exact_size(
        egui::vec2(EYE.x + 3.0 + galley.size().x, EYE.y.max(galley.size().y)),
        egui::Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        paint_eye_icon(
            ui.painter(),
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + EYE.x * 0.5, rect.center().y),
                EYE,
            ),
            color,
        );
        ui.painter().galley(
            egui::pos2(
                rect.left() + EYE.x + 3.0,
                rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            color,
        );
    }
    let label = format!("{} {read}/{}", i18n::LU_PAR.get(language), detail.audience);
    btn.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
    let btn = btn
        .on_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    egui::Popup::from_toggle_button_response(&btn)
        .id(popup_id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(160.0);
            let (delivered_lbl, read_lbl) = (
                crate::ui::i18n::RECU_PAR.get(language),
                crate::ui::i18n::LU_PAR.get(language),
            );
            ui.label(egui::RichText::new(delivered_lbl).strong());
            if detail.delivered_by.is_empty() {
                ui.label(egui::RichText::new("—").weak());
            } else {
                for name in &detail.delivered_by {
                    ui.label(name);
                }
            }
            ui.separator();
            ui.label(egui::RichText::new(read_lbl).strong());
            if detail.read_by.is_empty() {
                ui.label(egui::RichText::new("—").weak());
            } else {
                for name in &detail.read_by {
                    ui.label(name);
                }
            }
        });
}

/// Rend le corps d'un message (texte Markdown puis média éventuel). Les
/// messages très longs (`collapse` présent) s'affichent repliés : aperçu +
/// « Afficher la suite » ; dépliés, un bouton « Réduire » les referme.
/// Renvoie (action média éventuelle, bascule replié/déplié cliquée).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_message_body(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    parsed: &crate::ui::markdown::ParsedMarkdown,
    collapse: Option<&crate::ui::snapshot::CollapseInfo>,
    expanded: bool,
    language: UiLanguage,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &crate::ui::EmojiTextures,
    media_textures: &std::collections::HashMap<String, Option<egui::TextureHandle>>,
    media_progress: &std::collections::HashMap<String, crate::message::MediaProgress>,
) -> (Option<crate::ui::media::MediaAction>, bool) {
    let mut toggled = false;
    if !msg.content.is_empty() {
        match collapse {
            Some(info) if !expanded => {
                crate::ui::markdown::render_parsed_markdown(
                    ui,
                    &info.preview,
                    emoji_map,
                    emoji_textures,
                );
                let label = crate::ui::i18n::AFFICHER_LA_SUITE_MODELE
                    .get(language)
                    .replace("{lignes}", &info.total_lines.to_string())
                    .replace("{caracteres}", &info.total_chars.to_string());
                if ui.small_button(label).clicked() {
                    toggled = true;
                }
            }
            Some(_) => {
                crate::ui::markdown::render_parsed_markdown(ui, parsed, emoji_map, emoji_textures);
                let label = crate::ui::i18n::REDUIRE.get(language);
                if ui.small_button(label).clicked() {
                    toggled = true;
                }
            }
            None => {
                crate::ui::markdown::render_parsed_markdown(ui, parsed, emoji_map, emoji_textures);
            }
        }
    }
    if let Some(media) = &msg.media {
        // Pendant le transfert : barre de progression au lieu de la carte.
        if let Some(progress) = media_progress.get(&media.id) {
            crate::ui::media::render_media_progress(ui, media, progress);
            return (None, toggled);
        }
        let texture = media_textures.get(&media.id).and_then(|t| t.as_ref());
        return (
            crate::ui::media::render_media_block(ui, media, texture),
            toggled,
        );
    }
    (None, toggled)
}

/// Enregistre l'action média choisie (ouverture ou téléchargement) dans les
/// variables collectées pendant le rendu, traitées après la zone défilante.
pub(super) fn apply_media_action(
    action: crate::ui::media::MediaAction,
    msg: &ChatMessage,
    view_open: &mut Option<String>,
    download: &mut Option<(String, String)>,
) {
    let Some(media) = &msg.media else { return };
    match action {
        crate::ui::media::MediaAction::View => *view_open = Some(media.id.clone()),
        crate::ui::media::MediaAction::Download => {
            *download = Some((media.id.clone(), media.filename.clone()))
        }
    }
}

/// Peint la texture PNG d'un emoji dans `rect` (jamais un glyphe police, cf.
/// `emoji_picker::render_inline`), ou ne peint rien si la texture est absente
/// du registre (emoji inconnu/non chargé).
pub(super) fn paint_emoji_texture(
    ui: &egui::Ui,
    rect: egui::Rect,
    emoji: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &crate::ui::EmojiTextures,
) {
    if let Some(texture) = emoji_map
        .get(emoji)
        .and_then(|idx| emoji_textures.get(ui.ctx(), *idx))
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
pub(super) fn render_reaction_pills(
    ui: &mut egui::Ui,
    reactions: &[ReactionEntry],
    my_username: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &crate::ui::EmojiTextures,
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
                egui::Stroke::new(1.0, crate::ui::theme::palette(ui).surface_strong)
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
pub(super) const REPLY_QUOTE_AVATAR: f32 = 16.0;
/// Durée du flash de surlignage après un saut vers un message (secondes).
pub(super) const HIGHLIGHT_SECS: f32 = 2.0;

/// Citation compacte au-dessus d'un message qui répond à un autre (façon
/// Discord) : ligne de liaison qui part de l'avatar, mini-avatar et nom
/// coloré de l'auteur d'origine, extrait sur une seule ligne. Cliquable pour
/// remonter au message d'origine (renvoie `true` au clic). `resolved` est
/// `None` si le message d'origine a expiré du ring-buffer ou n'a jamais été
/// reçu par ce pair.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_reply_quote(
    ui: &mut egui::Ui,
    row_index: usize,
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
                        crate::ui::avatar::show_avatar(
                            ui,
                            author_avatar,
                            author_name,
                            REPLY_QUOTE_AVATAR,
                        );
                        ui.label(
                            egui::RichText::new(author_name)
                                .small()
                                .color(author_color)
                                .family(egui::FontFamily::Name(crate::ui::BOLD_FAMILY.into())),
                        );
                        if orig.media.is_some() {
                            crate::ui::media::render_reply_thumb(
                                ui,
                                media_texture,
                                REPLY_QUOTE_AVATAR,
                            );
                        }
                        let snippet = if orig.content.is_empty() && orig.media.is_some() {
                            "📎".to_string()
                        } else {
                            crate::ui::media::elide(&orig.content, 90)
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(snippet)
                                    .small()
                                    .color(crate::ui::theme::palette(ui).text_muted),
                            )
                            .truncate(),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new(not_found_label)
                                .small()
                                .italics()
                                .color(crate::ui::theme::palette(ui).text_muted),
                        );
                    }
                }
            })
            .response
            .rect;

        // Ligne de liaison : descend du coin arrondi vers l'avatar du message
        // qui répond, et rejoint horizontalement la citation.
        let stroke = egui::Stroke::new(2.0, crate::ui::theme::palette(ui).separator);
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
                egui::Id::new(("reply_quote", row_index)),
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
pub(super) struct HoverToolbarResult {
    /// Le pointeur est actuellement au-dessus de la barre elle-même (pour
    /// éviter que le survol ne « clignote » en passant de la ligne à la
    /// barre flottante qui la recouvre partiellement).
    pub(super) pointer_over_toolbar: bool,
    pub(super) reply_clicked: bool,
    pub(super) quick_emoji: Option<String>,
}
