use eframe::egui;

use super::{AboutTab, AbcomApp, ThemePreference, UiLanguage};

const LICENSE_TEXT: &str = include_str!("../../LICENSE");

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
        let service_name = "Abcom";
        let language_label = self.tr("Langue", "Language");
        let theme_label = self.tr("Theme", "Theme");
        let credits_label = self.tr("Credits", "Credits");
        let license_tab_label = self.tr("Licence", "License");
        let theme_system_label = self.tr("Suivre le systeme", "Follow system");
        let theme_light_label = self.tr("Clair", "Light");
        let theme_dark_label = self.tr("Sombre", "Dark");
        let panel_fill = ctx.style().visuals.panel_fill;
        let panel_stroke = ctx.style().visuals.widgets.noninteractive.bg_stroke;
        let info_title = self.tr("Informations", "Information");
        let about_label = self.tr("Service", "Service");
        let description_label = self.tr("Description", "Description");
        let copyright_label = self.tr("Copyright", "Copyright");
        let license_label = self.tr("Licence", "License");
        let version_label = self.tr("Version", "Version");
        let developers_label = self.tr("Developpeurs", "Developers");
        let description_text = self.tr(
            "Messagerie pair-a-pair locale avec decouverte automatique des pairs, conversations, groupes, aliases reseau et rendu Markdown natif.",
            "Local peer-to-peer messaging with automatic peer discovery, conversations, groups, network aliases, and native Markdown rendering.",
        );
        let warranty_text = self.tr(
            "Logiciel distribue sans garantie. Voir la licence AGPL v3 pour les details.",
            "Software distributed without warranty. See the AGPL v3 license for details.",
        );

        egui::TopBottomPanel::top("header_bar")
            .resizable(false)
            .exact_height(28.0)
            .frame(
                egui::Frame::NONE
                    .fill(panel_fill)
                    .stroke(panel_stroke)
                    .inner_margin(egui::Margin::symmetric(0, 0))
                    .outer_margin(egui::Margin::symmetric(0, 0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                ui.spacing_mut().button_padding = egui::vec2(8.0, 0.0);
                ui.set_height(ui.available_height());

                ui.horizontal_centered(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(credits_label).clicked() {
                            self.about_tab = AboutTab::Credits;
                            self.show_credits_modal = true;
                        }

                        if ui.button(license_tab_label).clicked() {
                            self.about_tab = AboutTab::License;
                            self.show_credits_modal = true;
                        }

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

        if self.show_credits_modal {
            let mut open = self.show_credits_modal;
            egui::Window::new(info_title)
                .open(&mut open)
                .resizable(true)
                .collapsible(false)
                .default_width(640.0)
                .default_height(480.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let credits_selected = self.about_tab == AboutTab::Credits;
                        if ui.selectable_label(credits_selected, credits_label).clicked() {
                            self.about_tab = AboutTab::Credits;
                        }
                        let license_selected = self.about_tab == AboutTab::License;
                        if ui
                            .selectable_label(license_selected, license_tab_label)
                            .clicked()
                        {
                            self.about_tab = AboutTab::License;
                        }
                    });
                    ui.separator();
                    ui.add_space(6.0);

                    match self.about_tab {
                        AboutTab::Credits => {
                            ui.label(egui::RichText::new(service_name).heading().strong());
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(format!("{}: {}", about_label, service_name))
                                    .strong(),
                            );
                            ui.label(format!("{}: {}", description_label, description_text));
                            ui.label(format!("{}: {}", version_label, version));
                            ui.label(format!(
                                "{}: Hugo Lagouardat Massiroles, Rudy Alves",
                                developers_label
                            ));
                            ui.label(format!("{}: Abnd © 2026", copyright_label));
                            ui.label(format!("{}: GNU Affero General Public License v3", license_label));
                            ui.label(egui::RichText::new(warranty_text).small().weak());
                        }
                        AboutTab::License => {
                            ui.label(
                                egui::RichText::new("GNU Affero General Public License v3")
                                    .strong()
                                    .heading(),
                            );
                            ui.add_space(6.0);
                            egui::ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(LICENSE_TEXT).monospace().size(12.0),
                                        )
                                        .wrap_mode(egui::TextWrapMode::Wrap),
                                    );
                                });
                        }
                    }
                });
            self.show_credits_modal = open;
        }
    }
}