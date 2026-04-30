use eframe::egui;

use super::{AbcomApp, AppView, ThemePreference, UiLanguage};

impl AbcomApp {
    pub(crate) fn apply_theme_preference(&mut self, ctx: &egui::Context) {
        let initial_dark_mode = self
            .system_dark_mode
            .get_or_insert_with(|| ctx.style().visuals.dark_mode);

        let dark_mode = match self.theme_preference {
            ThemePreference::System => *initial_dark_mode,
            ThemePreference::Light => false,
            ThemePreference::Dark => true,
        };

        ctx.set_visuals(if dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    }

    pub(crate) fn show_header_bar(&mut self, ctx: &egui::Context) {
        let version = env!("CARGO_PKG_VERSION");
        let language_label = self.tr("Langue", "Language");
        let theme_label = self.tr("Theme", "Theme");
        let view_label = match self.active_view {
            AppView::Chat => self.tr("Messagerie P2P locale", "Local P2P messaging"),
            AppView::Networks => self.tr(
                "Réseaux et identités locales",
                "Networks and local identities",
            ),
        };
        let theme_system_label = self.tr("Suivre le systeme", "Follow system");
        let theme_light_label = self.tr("Clair", "Light");
        let theme_dark_label = self.tr("Sombre", "Dark");

        egui::TopBottomPanel::top("header_bar")
            .resizable(false)
            .exact_height(32.0)
            .frame(
                egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(0, 0))
                    .outer_margin(egui::Margin::symmetric(0, 0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Abnd © 2026 • v{}", version))
                            .small()
                            .color(ui.visuals().strong_text_color()),
                    );
                    ui.label(
                        egui::RichText::new(view_label)
                        .small()
                        .weak(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button(theme_label, |ui| {
                            ui.set_min_width(180.0);
                            ui.radio_value(
                                &mut self.theme_preference,
                                ThemePreference::System,
                                theme_system_label,
                            );
                            ui.radio_value(
                                &mut self.theme_preference,
                                ThemePreference::Light,
                                theme_light_label,
                            );
                            ui.radio_value(
                                &mut self.theme_preference,
                                ThemePreference::Dark,
                                theme_dark_label,
                            );
                        });

                        ui.menu_button(language_label, |ui| {
                            ui.set_min_width(180.0);
                            ui.radio_value(&mut self.ui_language, UiLanguage::French, "Francais");
                            ui.radio_value(&mut self.ui_language, UiLanguage::English, "English");
                        });
                    });
                });
            });
    }
}