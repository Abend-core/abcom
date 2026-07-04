use eframe::egui;

use super::avatar::show_avatar;
use super::{AbcomApp, SettingsTab, ThemePreference, UiLanguage};

/// Diamètre de l'aperçu d'avatar dans l'onglet Profil.
const PROFILE_AVATAR_SIZE: f32 = 96.0;

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

        // `set_visuals` reconstruit tout le style : ne l'appliquer qu'au
        // changement effectif, pas à chaque frame.
        if self.applied_dark_mode == Some(dark_mode) {
            return;
        }
        self.applied_dark_mode = Some(dark_mode);
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
        let profile_label = self.tr("Profil", "Profile");
        let general_label = self.tr("Général", "General");
        let credits_label = self.tr("Crédits", "Credits");
        let license_label = self.tr("Licence", "License");

        // Onglet Profil
        let profile_heading = self.tr("Image de profil", "Profile picture");
        let profile_hint = self.tr(
            "Visible par les autres pairs dans les conversations.",
            "Visible to other peers in conversations.",
        );
        let choose_label = self.tr("Choisir une image…", "Choose an image…");
        let change_label = self.tr("Changer l'image…", "Change image…");
        let remove_label = self.tr("Retirer", "Remove");

        // Avatar courant (texture chargée paresseusement) calculé avant la
        // fenêtre pour éviter un double emprunt de `self` dans la closure.
        let my_name = self.state.lock().unwrap().my_username.clone();
        let avatar_texture = self.avatar_texture(ctx, &my_name);
        let has_avatar = self.state.lock().unwrap().my_avatar.is_some();
        let mut pick_avatar = false;
        let mut clear_avatar = false;

        // Onglet Général
        let language_label = self.tr("Langue", "Language");
        let theme_label = self.tr("Thème", "Theme");
        let theme_system_label = self.tr("Suivre le système", "Follow system");
        let theme_light_label = self.tr("Clair", "Light");
        let theme_dark_label = self.tr("Sombre", "Dark");

        // Onglet Crédits
        let description_text = self.tr(
            "Messagerie pair-à-pair locale : découverte automatique des pairs, conversations, groupes, alias de contacts et rendu Markdown natif.",
            "Local peer-to-peer messaging: automatic peer discovery, conversations, groups, contact aliases, and native Markdown rendering.",
        );
        let warranty_text = self.tr(
            "Logiciel distribué sans garantie. Voir la licence AGPL v3 pour les détails.",
            "Software distributed without warranty. See the AGPL v3 license for details.",
        );
        let klipy_role = self.tr(
            "GIF animés, mèmes statiques et stickers dans le sélecteur de contenu.",
            "Animated GIFs, static memes, and stickers in the content picker.",
        );
        let openmoji_role = self.tr(
            "Jeu d'emojis utilisé dans le picker et l'affichage inline des messages.",
            "Emoji set used in the emoji picker and inline message rendering.",
        );
        let inter_role = self.tr(
            "Police d'écriture en gras utilisée pour les noms d'auteur dans les messages.",
            "Bold typeface used for author names in messages.",
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
                        (SettingsTab::Profile, profile_label),
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
                    SettingsTab::Profile => {
                        ui.label(egui::RichText::new(profile_heading).strong());
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            show_avatar(ui, avatar_texture.as_ref(), &my_name, PROFILE_AVATAR_SIZE);
                            ui.add_space(16.0);
                            ui.vertical(|ui| {
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(&my_name).heading().strong());
                                ui.add_space(2.0);
                                ui.label(egui::RichText::new(profile_hint).small().weak());
                                ui.add_space(10.0);
                                ui.horizontal(|ui| {
                                    let button_label = if has_avatar {
                                        change_label
                                    } else {
                                        choose_label
                                    };
                                    if ui.button(button_label).clicked() {
                                        pick_avatar = true;
                                    }
                                    if has_avatar && ui.button(remove_label).clicked() {
                                        clear_avatar = true;
                                    }
                                });
                            });
                        });
                    }
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
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                // ── Abcom ────────────────────────────────────
                                ui.label(egui::RichText::new(service_name).strong().heading());
                                ui.add_space(4.0);
                                egui::Grid::new("credits_abcom")
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        let lbl = |ui: &mut egui::Ui, t: &str| {
                                            ui.label(egui::RichText::new(t).strong());
                                        };
                                        lbl(ui, self.tr("Version", "Version"));
                                        ui.label(version);
                                        ui.end_row();
                                        lbl(ui, self.tr("Description", "Description"));
                                        ui.add(
                                            egui::Label::new(description_text)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.tr("Développeurs", "Developers"));
                                        ui.label("Hugo Lagouardat Massiroles, Rudy Alves");
                                        ui.end_row();
                                        lbl(ui, self.tr("Copyright", "Copyright"));
                                        ui.label("Abnd © 2026");
                                        ui.end_row();
                                        lbl(ui, self.tr("Licence", "License"));
                                        ui.label("GNU Affero General Public License v3");
                                        ui.end_row();
                                    });
                                ui.add_space(2.0);
                                ui.label(egui::RichText::new(warranty_text).small().weak());

                                ui.add_space(14.0);
                                ui.separator();
                                ui.add_space(8.0);

                                // ── Klipy ────────────────────────────────────
                                ui.label(egui::RichText::new("Klipy").strong().heading());
                                ui.add_space(4.0);
                                egui::Grid::new("credits_klipy")
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        let lbl = |ui: &mut egui::Ui, t: &str| {
                                            ui.label(egui::RichText::new(t).strong());
                                        };
                                        lbl(ui, self.tr("Fournisseur", "Provider"));
                                        ui.label("KLIPY (klipy.com)");
                                        ui.end_row();
                                        lbl(ui, self.tr("Rôle", "Role"));
                                        ui.add(
                                            egui::Label::new(klipy_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.tr("Licence", "License"));
                                        ui.label("Klipy API Terms of Service");
                                        ui.end_row();
                                        lbl(ui, self.tr("Attribution", "Attribution"));
                                        ui.label("© KLIPY — Powered by KLIPY");
                                        ui.end_row();
                                    });

                                ui.add_space(14.0);
                                ui.separator();
                                ui.add_space(8.0);

                                // ── OpenEmoji ────────────────────────────────
                                ui.label(egui::RichText::new("OpenEmoji").strong().heading());
                                ui.add_space(4.0);
                                egui::Grid::new("credits_openmoji")
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        let lbl = |ui: &mut egui::Ui, t: &str| {
                                            ui.label(egui::RichText::new(t).strong());
                                        };
                                        lbl(ui, self.tr("Source", "Source"));
                                        ui.label("OpenEmoji (openmoji.org)");
                                        ui.end_row();
                                        lbl(ui, self.tr("Rôle", "Role"));
                                        ui.add(
                                            egui::Label::new(openmoji_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.tr("Licence", "License"));
                                        ui.label("CC BY 4.0");
                                        ui.end_row();
                                        lbl(ui, self.tr("Auteurs", "Authors"));
                                        ui.label("HfG Schwäbisch Gmünd & contributeurs OpenEmoji");
                                        ui.end_row();
                                    });

                                ui.add_space(14.0);
                                ui.separator();
                                ui.add_space(8.0);

                                // ── Inter ────────────────────────────────────
                                ui.label(
                                    egui::RichText::new("Inter (police d'écriture)")
                                        .strong()
                                        .heading(),
                                );
                                ui.add_space(4.0);
                                egui::Grid::new("credits_inter")
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        let lbl = |ui: &mut egui::Ui, t: &str| {
                                            ui.label(egui::RichText::new(t).strong());
                                        };
                                        lbl(ui, self.tr("Auteur", "Author"));
                                        ui.label("Rasmus Andersson");
                                        ui.end_row();
                                        lbl(ui, self.tr("Rôle", "Role"));
                                        ui.add(
                                            egui::Label::new(inter_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.tr("Licence", "License"));
                                        ui.label("SIL Open Font License v1.1");
                                        ui.end_row();
                                    });
                                ui.add_space(8.0);
                            });
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

        // Application différée des actions de l'onglet Profil (hors closure pour
        // éviter tout emprunt concurrent de `self`).
        if pick_avatar {
            // Le sélecteur natif est ouvert à la frame suivante (cf. `update`).
            self.pending_avatar_pick = true;
        }
        if clear_avatar {
            self.state.lock().unwrap().clear_my_avatar();
            self.avatar_textures.remove(&my_name);
            self.broadcast_my_avatar();
        }
    }
}
