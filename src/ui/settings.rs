use super::i18n;
use eframe::egui;

use crate::util::MutexExt;

use super::avatar::show_avatar;
use super::{AbcomApp, SettingsTab, UiLanguage};

/// Diamètre de l'aperçu d'avatar dans l'onglet Profil.
const PROFILE_AVATAR_SIZE: f32 = 96.0;

const LICENSE_TEXT: &str = include_str!("../../LICENSE");

impl AbcomApp {
    /// Fenêtre Paramètres : regroupe langue, thème, crédits et licence.
    /// Ouverte depuis l'icône engrenage en bas de la barre latérale, elle
    /// remplace l'ancien bandeau supérieur. Un bandeau d'onglets permet de
    /// naviguer entre Général, Crédits et Licence.
    pub(crate) fn render_settings(&mut self, ctx: &egui::Context) {
        if !self.modals.settings_open {
            return;
        }

        let version = env!("CARGO_PKG_VERSION");
        let service_name = "Abcom";

        let title = self.t(i18n::PARAMETRES);
        let profile_label = self.t(i18n::PROFIL);
        let general_label = self.t(i18n::GENERAL);
        let credits_label = self.t(i18n::CREDITS);
        let license_label = self.t(i18n::LICENCE);

        // Onglet Profil
        let profile_heading = self.t(i18n::IMAGE_DE_PROFIL);
        let profile_hint = self.t(i18n::VISIBLE_PAR_LES_AUTRES_PAIRS_DANS);
        let choose_label = self.t(i18n::CHOISIR_UNE_IMAGE);
        let change_label = self.t(i18n::CHANGER_L_IMAGE);
        let remove_label = self.t(i18n::RETIRER);

        // Avatar courant (texture chargée paresseusement) calculé avant la
        // fenêtre pour éviter un double emprunt de `self` dans la closure.
        let my_name = self.state.lock_safe().my_username.clone();
        let avatar_texture = self.avatar_texture(ctx, &my_name);
        let has_avatar = self.state.lock_safe().my_avatar.is_some();
        let mut pick_avatar = false;
        let mut clear_avatar = false;

        // Onglet Général
        let language_label = self.t(i18n::LANGUE);
        let theme_label = self.t(i18n::THEME);
        let theme_system_label = self.t(i18n::SUIVRE_LE_SYSTEME);
        let theme_light_label = self.t(i18n::CLAIR);
        let theme_dark_label = self.t(i18n::SOMBRE);

        // Onglet Crédits
        let description_text = self.t(i18n::MESSAGERIE_PAIR_A_PAIR_LOCALE_DECOUVERTE);
        let warranty_text = self.t(i18n::LOGICIEL_DISTRIBUE_SANS_GARANTIE_VOIR_LA);
        let klipy_role = self.t(i18n::GIF_ANIMES_MEMES_STATIQUES_ET_STICKERS);
        let openmoji_role = self.t(i18n::JEU_D_EMOJIS_UTILISE_DANS_LE);
        let inter_role = self.t(i18n::POLICE_D_ECRITURE_EN_GRAS_UTILISEE);

        // Taille fixe (celle, maximale, de l'onglet Licence) pour que la fenêtre
        // ne change pas de dimensions quand on bascule d'un onglet à l'autre.
        const SETTINGS_SIZE: egui::Vec2 = egui::vec2(640.0, 480.0);

        let mut open = self.modals.settings_open;
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
                            .selectable_label(self.modals.settings_tab == tab, label)
                            .clicked()
                        {
                            self.modals.settings_tab = tab;
                        }
                    }
                });
                ui.separator();
                ui.add_space(8.0);

                match self.modals.settings_tab {
                    SettingsTab::Profile => {
                        ui.label(egui::RichText::new(profile_heading).strong());
                        ui.add_space(8.0);
                        // Empreinte de la clé d'identité (Noise) : à comparer
                        // hors-bande avec un pair pour vérifier l'appairage.
                        ui.label(
                            egui::RichText::new(format!(
                                "{} : {}",
                                self.t(i18n::EMPREINTE_DE_VOTRE_CLE),
                                self.identity_fingerprint
                            ))
                            .small()
                            .weak(),
                        );
                        let psk_label = if self.psk_active {
                            self.t(i18n::PASSPHRASE_DE_SALON_ACTIVE)
                        } else {
                            self.t(i18n::PASSPHRASE_DE_SALON_DESACTIVEE_ABCOM_PASSPHRASE)
                        };
                        ui.label(egui::RichText::new(psk_label).small().weak());
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
                                        egui::ThemePreference::System,
                                        theme_system_label,
                                    );
                                    ui.radio_value(
                                        &mut self.theme_preference,
                                        egui::ThemePreference::Light,
                                        theme_light_label,
                                    );
                                    ui.radio_value(
                                        &mut self.theme_preference,
                                        egui::ThemePreference::Dark,
                                        theme_dark_label,
                                    );
                                });
                                ui.end_row();

                                ui.label(egui::RichText::new(self.t(i18n::NOTIFICATIONS)).strong());
                                {
                                    let label = self.t(i18n::AFFICHER_UN_APERCU_DU_MESSAGE);
                                    if ui.checkbox(&mut self.notif_preview, label).changed() {
                                        let v = if self.notif_preview { "1" } else { "0" };
                                        self.state.lock_safe().set_pref("notif_preview", v);
                                    }
                                }
                                ui.end_row();

                                ui.label(egui::RichText::new(self.t(i18n::DEMARRAGE)).strong());
                                {
                                    let label = self.t(i18n::LANCER_ABCOM_A_L_OUVERTURE_DE);
                                    if ui.checkbox(&mut self.autostart_enabled, label).changed() {
                                        match crate::platform::autostart::set_enabled(
                                            self.autostart_enabled,
                                        ) {
                                            Ok(()) => {
                                                let v =
                                                    if self.autostart_enabled { "1" } else { "0" };
                                                self.state.lock_safe().set_pref("autostart", v);
                                            }
                                            Err(e) => {
                                                tracing::warn!("échec autostart : {e}");
                                                self.autostart_enabled = !self.autostart_enabled;
                                            }
                                        }
                                    }
                                }
                                ui.end_row();

                                ui.label(egui::RichText::new(self.t(i18n::DONNEES)).strong());
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(self.t(i18n::EXPORTER_LA_CONVERSATION_2))
                                        .clicked()
                                    {
                                        self.pending_export = true;
                                    }
                                    if ui.button(self.t(i18n::COMPACTER_LA_BASE)).clicked() {
                                        self.state.lock_safe().compact_storage();
                                        self.last_notification = Some(
                                            self.t(i18n::COMPACTION_DE_LA_BASE_LANCEE).to_string(),
                                        );
                                        self.notification_time = std::time::Instant::now();
                                    }
                                });
                                ui.end_row();

                                // Diagnostic : compteurs de la session en cours
                                // (cf. `metrics`). « Jetés » non nul = file
                                // saturée ou pair injoignable.
                                ui.label(egui::RichText::new(self.t(i18n::DIAGNOSTIC)).strong());
                                {
                                    let m = crate::metrics::snapshot();
                                    let line = format!(
                                        "{} {} · {} {} · {} {} · {} {}",
                                        self.t(i18n::ENVOYES),
                                        m.packets_sent,
                                        self.t(i18n::RECUS),
                                        m.packets_received,
                                        self.t(i18n::JETES),
                                        m.packets_dropped,
                                        self.t(i18n::PAIRS_VUS),
                                        m.peers_seen,
                                    );
                                    ui.label(egui::RichText::new(line).small().weak());
                                }
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
                                        lbl(ui, self.t(i18n::VERSION));
                                        ui.label(version);
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::DESCRIPTION));
                                        ui.add(
                                            egui::Label::new(description_text)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::DEVELOPPEURS));
                                        ui.label("Hugo Lagouardat Massiroles, Rudy Alves");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::COPYRIGHT));
                                        ui.label("Abnd © 2026");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::LICENCE));
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
                                        lbl(ui, self.t(i18n::FOURNISSEUR));
                                        ui.label("KLIPY (klipy.com)");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::ROLE));
                                        ui.add(
                                            egui::Label::new(klipy_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::LICENCE));
                                        ui.label("Klipy API Terms of Service");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::ATTRIBUTION));
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
                                        lbl(ui, self.t(i18n::SOURCE));
                                        ui.label("OpenEmoji (openmoji.org)");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::ROLE));
                                        ui.add(
                                            egui::Label::new(openmoji_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::LICENCE));
                                        ui.label("CC BY 4.0");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::AUTEURS));
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
                                        lbl(ui, self.t(i18n::AUTEUR));
                                        ui.label("Rasmus Andersson");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::ROLE));
                                        ui.add(
                                            egui::Label::new(inter_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::LICENCE));
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
        self.modals.settings_open = open;

        // Application différée des actions de l'onglet Profil (hors closure pour
        // éviter tout emprunt concurrent de `self`).
        if pick_avatar {
            // Le sélecteur natif est ouvert à la frame suivante (cf. `update`).
            self.pending_avatar_pick = true;
        }
        if clear_avatar {
            self.state.lock_safe().clear_my_avatar();
            self.avatar_textures.remove(&my_name);
            self.broadcast_my_avatar();
        }
    }
}
