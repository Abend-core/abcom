//! Barre d'actions flottante affichée au survol d'un message.

use eframe::egui;

use super::row::{paint_emoji_texture, HoverToolbarResult, HOVER_BTN_SIZE, HOVER_EMOJI_SIZE};
use crate::ui::AbcomApp;

impl AbcomApp {
    /// Barre d'actions flottante affichée au survol d'un message : emojis
    /// récents, "+" (picker complet de réaction) et "répondre". Pas de
    /// bouton de transfert. `row_rect` est le rectangle pleine largeur de la
    /// ligne : la barre est collée au bord droit du fil (position stable
    /// quel que soit le message) et chevauche le haut de la ligne, façon
    /// Discord.
    pub(super) fn show_hover_toolbar(
        &mut self,
        ctx: &egui::Context,
        row_index: usize,
        msg_hash: u64,
        row_rect: egui::Rect,
        reply_label: &str,
        add_reaction_label: &str,
    ) -> HoverToolbarResult {
        // Seuls les emojis connus du registre : la texture est décodée au premier rendu.
        let emojis: Vec<String> = self
            .recent_reaction_emojis
            .iter()
            .filter(|e| self.emoji.map.contains_key(*e))
            .cloned()
            .collect();

        let toolbar_w = HOVER_BTN_SIZE * (emojis.len() as f32 + 2.0) + 6.0;
        let toolbar_h = HOVER_BTN_SIZE + 10.0;
        let anchor = egui::pos2(
            row_rect.right() - toolbar_w - 12.0,
            row_rect.top() - toolbar_h * 0.5,
        );

        let mut quick_emoji = None;
        let mut reply_clicked = false;
        let mut plus_rect = None;

        let area = egui::Area::new(egui::Id::new(("msg_hover_toolbar", row_index)))
            .order(egui::Order::Foreground)
            .fixed_pos(anchor);
        let resp = area.show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for ch in &emojis {
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
                                &self.emoji.map,
                                &self.emoji.textures,
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
