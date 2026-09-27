//! Boîtes de dialogue modales.
//!
//! `egui::Window` n'empêche rien derrière elle : on pouvait écrire dans le
//! composeur, changer de conversation ou déclencher une action de la barre
//! latérale pendant que les réglages ou la création de salon étaient ouverts.
//! `egui::Modal` pose un fond qui assombrit l'application, avale les clics et
//! réserve le clavier à la boîte.

use eframe::egui;

/// Croix de fermeture peinte : le glyphe « ✕ » n'est pas rendu de façon fiable
/// par les polices embarquées (même raison que `chip_remove_button`).
fn close_button(ui: &mut egui::Ui, label: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() {
            crate::ui::theme::palette(ui).danger
        } else {
            crate::ui::theme::palette(ui).text_muted
        };
        let stroke = egui::Stroke::new(1.8, color);
        let c = rect.center();
        let d = 5.0;
        let p = ui.painter();
        p.line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], stroke);
        p.line_segment([c + egui::vec2(d, -d), c + egui::vec2(-d, d)], stroke);
    }
    // Bouton entièrement peint : sans ceci, il est invisible pour un lecteur
    // d'écran et introuvable pour les tests pilotés par AccessKit.
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp.on_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// Boîte modale centrée : bandeau de titre, croix de fermeture, puis le
/// contenu.
pub(crate) struct Modal<'a> {
    id: &'a str,
    title: &'a str,
    close_label: &'a str,
    width: f32,
    height: Option<f32>,
}

impl<'a> Modal<'a> {
    pub(crate) fn new(id: &'a str, title: &'a str, close_label: &'a str, width: f32) -> Self {
        Self {
            id,
            title,
            close_label,
            width,
            height: None,
        }
    }

    /// Fixe la hauteur totale. Sans elle, la boîte s'ajuste à son contenu — la
    /// borner évite qu'une zone de défilement `auto_shrink(false)` s'étire sur
    /// toute la fenêtre, faute de contrainte venant du parent.
    pub(crate) fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// Ne dessine rien et renvoie `None` quand `open` est faux. `open` retombe
    /// à faux sur la croix, un clic sur le fond ou Échap.
    pub(crate) fn show<R>(
        self,
        ctx: &egui::Context,
        open: &mut bool,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<R> {
        if !*open {
            return None;
        }

        let Self {
            id,
            title,
            close_label,
            width,
            height,
        } = self;

        let response = egui::Modal::new(egui::Id::new(id)).show(ctx, |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            if let Some(height) = height {
                ui.set_min_height(height);
                ui.set_max_height(height);
            }

            let mut close = false;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(title).heading());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = close_button(ui, close_label);
                });
            });
            ui.separator();
            ui.add_space(4.0);

            (content(ui), close)
        });

        // Échap est déjà consommé en amont par `close_topmost_overlay` ; il
        // reste le clic sur le fond. Évalué avant de sortir `inner`.
        let close = response.inner.1 || response.should_close();
        if close {
            *open = false;
        }
        Some(response.inner.0)
    }
}
