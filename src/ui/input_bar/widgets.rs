//! Petits widgets de la barre de saisie : chips de pièces jointes, boutons
//! peints et menu « + ».

use eframe::egui;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

pub(super) enum AttachmentMenuAction {
    AddFiles,
    AddFolder,
}

pub(super) const ACTION_BUTTON_SIZE: [f32; 2] = [28.0, 28.0];

pub(super) fn should_send_message(
    pressed_enter: bool,
    pressed_enter_fallback: bool,
    shortcode_menu_open: bool,
    input: &str,
) -> bool {
    (pressed_enter || (pressed_enter_fallback && !shortcode_menu_open)) && !input.trim().is_empty()
}

#[cfg(test)]
pub(super) fn push_unique_paths(
    target: &mut Vec<PathBuf>,
    paths: impl IntoIterator<Item = PathBuf>,
) {
    for path in paths {
        if !target.iter().any(|existing| existing == &path) {
            target.push(path);
        }
    }
}

pub(super) fn attachment_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

/// Chip de pièce jointe à largeur fixe : icône, nom tronqué (chemin complet
/// en infobulle) et croix de retrait collée à droite. Renvoie `true` si la
/// croix est cliquée. La largeur fixe permet une grille qui se replie sur
/// plusieurs lignes sans jamais déborder.
pub(super) fn attachment_chip(ui: &mut egui::Ui, path: &Path, width: f32) -> bool {
    let mut removed = false;
    egui::Frame::default()
        .fill(crate::ui::theme::palette(ui).surface_strong)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_width(width - 16.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                ui.label(if path.is_dir() { "📁" } else { "📄" });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if chip_remove_button(ui) {
                        removed = true;
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(attachment_label(path))
                                    .small()
                                    .color(crate::ui::theme::palette(ui).text),
                            )
                            .truncate(),
                        )
                        .on_hover_text(path.display().to_string());
                    });
                });
            });
        });
    removed
}

pub(super) fn action_button_chrome(ui: &egui::Ui, selected: bool) -> (egui::Color32, egui::Stroke) {
    let fill = if selected {
        crate::ui::theme::palette(ui).accent_soft
    } else {
        crate::ui::theme::palette(ui).surface_strong
    };
    let stroke = if selected {
        egui::Stroke::new(1.0, crate::ui::theme::palette(ui).accent_soft)
    } else {
        egui::Stroke::new(1.0, crate::ui::theme::palette(ui).separator)
    };
    (fill, stroke)
}

pub(super) fn action_button(
    ui: &mut egui::Ui,
    label: egui::RichText,
    tooltip: &str,
    selected: bool,
) -> egui::Response {
    let (fill, stroke) = action_button_chrome(ui, selected);
    ui.add_sized(
        ACTION_BUTTON_SIZE,
        egui::Button::new(label)
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(10)),
    )
    .on_hover_text(tooltip)
}

pub(super) fn icon_button(
    ui: &mut egui::Ui,
    tooltip: &str,
    selected: bool,
    paint: impl FnOnce(&egui::Painter, egui::Rect, egui::Color32),
) -> egui::Response {
    let (fill, stroke) = action_button_chrome(ui, selected);
    let response = ui
        .add_sized(
            ACTION_BUTTON_SIZE,
            egui::Button::new(egui::RichText::new(""))
                .fill(fill)
                .stroke(stroke)
                .corner_radius(egui::CornerRadius::same(10)),
        )
        .on_hover_text(tooltip);
    // Bouton sans texte : l'infobulle sert de libellé accessible.
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tooltip));
    paint(
        ui.painter(),
        response.rect.shrink2(egui::vec2(7.0, 7.0)),
        crate::ui::theme::palette(ui).text,
    );
    response
}

/// Petite croix peinte pour retirer une pièce jointe (glyphe « ✕ » non rendu de
/// façon fiable par la police). Renvoie `true` au clic.
pub(super) fn chip_remove_button(ui: &mut egui::Ui) -> bool {
    let accessible_label = "Retirer la pièce jointe";
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() {
            crate::ui::theme::palette(ui).danger
        } else {
            crate::ui::theme::palette(ui).text_muted
        };
        let stroke = egui::Stroke::new(1.6, color);
        let c = rect.center();
        let d = 3.5;
        let p = ui.painter();
        p.line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], stroke);
        p.line_segment([c + egui::vec2(d, -d), c + egui::vec2(-d, d)], stroke);
    }
    // Sans ceci, un bouton entièrement peint est invisible pour un lecteur
    // d'écran — et introuvable pour les tests pilotés par AccessKit.
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, accessible_label)
    });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

pub(super) fn paint_plus_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    let arm = rect.width().min(rect.height()) * 0.34;
    let stroke = egui::Stroke::new(2.0, color);
    painter.line_segment(
        [
            egui::pos2(center.x - arm, center.y),
            egui::pos2(center.x + arm, center.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - arm),
            egui::pos2(center.x, center.y + arm),
        ],
        stroke,
    );
}

pub(super) fn paint_send_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let left = rect.left() + 1.0;
    let right = rect.right() - 1.0;
    let top = rect.top() + 2.0;
    let bottom = rect.bottom() - 2.0;
    let center_y = rect.center().y;
    let tip = egui::pos2(right, center_y);
    let tail = egui::pos2(left + 1.0, center_y);
    let stroke = egui::Stroke::new(2.2, color);
    painter.line_segment([tail, tip], stroke);
    painter.line_segment([egui::pos2(right - 6.5, top), tip], stroke);
    painter.line_segment([egui::pos2(right - 6.5, bottom), tip], stroke);
}

/// Menu « + ». Renvoie l'action choisie et le rectangle **réellement** occupé
/// par la popup : c'est lui qui décide si un clic tombe dedans ou dehors, une
/// estimation ratant la cible refermait le menu sur l'appui, avant que le
/// bouton visé n'ait vu le relâchement — le clic était perdu.
pub(super) fn attachment_menu_popup(
    ctx: &egui::Context,
    anchor_rect: egui::Rect,
    add_files_label: &str,
    add_folder_label: &str,
) -> (Option<AttachmentMenuAction>, egui::Rect) {
    // Use the size remembered from the previous frame (or a safe default) so that
    // the popup's bottom-left is anchored just above the + button.
    let popup_id = egui::Id::new("attachment_menu_popup");
    let popup_h = ctx
        .memory(|m| m.area_rect(popup_id))
        .map(|r| r.height())
        .unwrap_or(80.0);
    let popup_pos = anchor_rect.left_top() - egui::vec2(0.0, popup_h + 6.0);

    let area = egui::Area::new(popup_id)
        .order(egui::Order::Foreground)
        .fixed_pos(popup_pos);

    let shown = area.show(ctx, |ui| {
        egui::Frame::popup(ui.style())
            .fill(crate::ui::theme::palette(ui).surface)
            .stroke(egui::Stroke::new(
                1.0,
                crate::ui::theme::palette(ui).separator,
            ))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                let mut action = None;
                ui.set_min_width(200.0);
                if ui.button(add_files_label).clicked() {
                    action = Some(AttachmentMenuAction::AddFiles);
                }
                if ui.button(add_folder_label).clicked() {
                    action = Some(AttachmentMenuAction::AddFolder);
                }
                action
            })
            .inner
    });

    // Hauteur mesurée différente de celle supposée : la popup est posée un cran
    // trop haut pour cette frame. Sans repeinture, egui n'en programme aucune et
    // elle y resterait jusqu'au prochain événement.
    if (shown.response.rect.height() - popup_h).abs() > 0.5 {
        ctx.request_repaint();
    }

    (shown.inner, shown.response.rect)
}
