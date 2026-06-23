use eframe::egui;

use super::{AbcomApp, SettingsTab, ThemePreference, UiLanguage};

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

    /// Fenêtre Paramètres : regroupe langue, thème, crédits et licence.
    /// Ouverte depuis l'icône engrenage en bas de la barre latérale, elle
    /// remplace l'ancien bandeau supérieur. Un bandeau d'onglets permet de
    /// naviguer entre Général, Crédits et Licence.
    pub(crate) fn render_settings(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let version = env!("CARGO_PKG_VERSION");
        let service_name = "Abcom";

        let title = self.tr("Paramètres", "Settings");
        let general_label = self.tr("Général", "General");
        let credits_label = self.tr("Crédits", "Credits");
        let license_label = self.tr("Licence", "License");

        // Onglet Général
        let language_label = self.tr("Langue", "Language");
        let theme_label = self.tr("Thème", "Theme");
        let theme_system_label = self.tr("Suivre le système", "Follow system");
        let theme_light_label = self.tr("Clair", "Light");
        let theme_dark_label = self.tr("Sombre", "Dark");

        // Onglet Crédits
        let service_field = self.tr("Service", "Service");
        let description_field = self.tr("Description", "Description");
        let version_field = self.tr("Version", "Version");
        let developers_field = self.tr("Développeurs", "Developers");
        let copyright_field = self.tr("Copyright", "Copyright");
        let license_field = self.tr("Licence", "License");
        let description_text = self.tr(
            "Messagerie pair-à-pair locale : découverte automatique des pairs, conversations, groupes, alias de contacts et rendu Markdown natif.",
            "Local peer-to-peer messaging: automatic peer discovery, conversations, groups, contact aliases, and native Markdown rendering.",
        );
        let warranty_text = self.tr(
            "Logiciel distribué sans garantie. Voir la licence AGPL v3 pour les détails.",
            "Software distributed without warranty. See the AGPL v3 license for details.",
        );

        // Taille fixe (celle, maximale, de l'onglet Licence) pour que la fenêtre
        // ne change pas de dimensions quand on bascule d'un onglet à l'autre.
        const SETTINGS_SIZE: egui::Vec2 = egui::vec2(640.0, 480.0);

        let mut open = self.show_settings;
        egui::Window::new(title)
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .fixed_size(SETTINGS_SIZE)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                // `fixed_size` borne la zone disponible mais, la fenêtre n'étant
                // pas redimensionnable, egui la rétracterait à la hauteur du
                // contenu. On force donc le contenu à remplir toute la zone pour
                // que la fenêtre garde la même taille sur tous les onglets.
                ui.set_min_size(ui.available_size());

                // Bandeau d'onglets
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (SettingsTab::General, general_label),
                        (SettingsTab::Credits, credits_label),
                        (SettingsTab::License, license_label),
                    ] {
                        if ui
                            .selectable_label(self.settings_tab == tab, label)
                            .clicked()
                        {
                            self.settings_tab = tab;
                        }
                    }
                });
                ui.separator();
                ui.add_space(8.0);

                match self.settings_tab {
                    SettingsTab::General => {
                        egui::Grid::new("settings_general")
                            .num_columns(2)
                            .spacing([16.0, 12.0])
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(language_label).strong());
                                ui.horizontal(|ui| {
                                    ui.radio_value(
                                        &mut self.ui_language,
                                        UiLanguage::French,
                                        "Français",
                                    );
                                    ui.radio_value(
                                        &mut self.ui_language,
                                        UiLanguage::English,
                                        "English",
                                    );
                                });
                                ui.end_row();

                                ui.label(egui::RichText::new(theme_label).strong());
                                ui.horizontal(|ui| {
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
                                ui.end_row();
                            });
                    }
                    SettingsTab::Credits => {
                        ui.label(egui::RichText::new(service_name).heading().strong());
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!("{}: {}", service_field, service_name))
                                .strong(),
                        );
                        ui.label(format!("{}: {}", description_field, description_text));
                        ui.label(format!("{}: {}", version_field, version));
                        ui.label(format!(
                            "{}: Hugo Lagouardat Massiroles, Rudy Alves",
                            developers_field
                        ));
                        ui.label(format!("{}: Abnd © 2026", copyright_field));
                        ui.label(format!(
                            "{}: GNU Affero General Public License v3",
                            license_field
                        ));
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(warranty_text).small().weak());
                    }
                    SettingsTab::License => {
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
        self.show_settings = open;
    }
}
