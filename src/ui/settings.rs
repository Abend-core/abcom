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
        let storage_label = self.t(i18n::STOCKAGE);
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
        // Le scan parcourt des dossiers : demandé après la fenêtre, hors de
        // la closure qui emprunte déjà `self`.
        let mut storage_actions = StorageActions::default();

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
        let symbols_role = self.t(i18n::POLICE_DE_SYMBOLES_UTILISEE_EN_REPLI);
        let noto_sans_role = self.t(i18n::POLICE_DE_TEXTE_DE_L_INTERFACE);
        let unifont_role = self.t(i18n::POLICE_DE_DERNIER_RECOURS);

        // Taille fixe (celle, maximale, de l'onglet Licence) pour que la fenêtre
        // ne change pas de dimensions quand on bascule d'un onglet à l'autre.
        const SETTINGS_SIZE: egui::Vec2 = egui::vec2(640.0, 480.0);

        let mut open = self.modals.settings_open;
        super::dialog::Modal::new("settings_modal", title, self.t(i18n::FERMER), SETTINGS_SIZE.x)
            .height(SETTINGS_SIZE.y)
            .show(ctx, &mut open, |ui| {
                // Bandeau d'onglets
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (SettingsTab::Profile, profile_label),
                        (SettingsTab::General, general_label),
                        (SettingsTab::Storage, storage_label),
                        (SettingsTab::Credits, credits_label),
                        (SettingsTab::License, license_label),
                    ] {
                        if ui
                            .selectable_label(self.modals.settings_tab == tab, label)
                            .clicked()
                        {
                            // Revenir sur Stockage doit repartir du disque : le
                            // dossier a pu changer depuis le dernier calcul, et
                            // un aperçu périmé activerait « Purger » à tort.
                            if tab == SettingsTab::Storage {
                                storage_actions.refresh = true;
                                storage_actions.preview = true;
                            }
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
                    SettingsTab::Storage => {
                        storage_tab(
                            ui,
                            self.storage_usage.as_ref(),
                            self.purge_preview.as_ref(),
                            &mut self.retention_days,
                            &mut self.purge_includes_images,
                            self.ui_language,
                            &mut storage_actions,
                        );
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

                                // ── Noto Sans ────────────────────────────────
                                ui.label(
                                    egui::RichText::new("Noto Sans (police de texte)")
                                        .strong()
                                        .heading(),
                                );
                                ui.add_space(4.0);
                                egui::Grid::new("credits_noto_sans")
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        let lbl = |ui: &mut egui::Ui, t: &str| {
                                            ui.label(egui::RichText::new(t).strong());
                                        };
                                        lbl(ui, self.t(i18n::AUTEURS));
                                        ui.label("The Noto Project Authors");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::ROLE));
                                        ui.add(
                                            egui::Label::new(noto_sans_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::LICENCE));
                                        ui.label("SIL Open Font License v1.1");
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

                                ui.add_space(14.0);
                                ui.separator();
                                ui.add_space(8.0);

                                // ── Noto Sans Symbols 2 ──────────────────────
                                ui.label(
                                    egui::RichText::new("Noto Sans Symbols 2 (police de symboles)")
                                        .strong()
                                        .heading(),
                                );
                                ui.add_space(4.0);
                                egui::Grid::new("credits_noto_symbols")
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        let lbl = |ui: &mut egui::Ui, t: &str| {
                                            ui.label(egui::RichText::new(t).strong());
                                        };
                                        lbl(ui, self.t(i18n::AUTEURS));
                                        ui.label("The Noto Project Authors");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::ROLE));
                                        ui.add(
                                            egui::Label::new(symbols_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::LICENCE));
                                        ui.label("SIL Open Font License v1.1");
                                        ui.end_row();
                                    });

                                ui.add_space(14.0);
                                ui.separator();
                                ui.add_space(8.0);

                                // ── Unifont ──────────────────────────────────
                                ui.label(
                                    egui::RichText::new("GNU Unifont (dernier recours)")
                                        .strong()
                                        .heading(),
                                );
                                ui.add_space(4.0);
                                egui::Grid::new("credits_unifont")
                                    .num_columns(2)
                                    .spacing([12.0, 4.0])
                                    .show(ui, |ui| {
                                        let lbl = |ui: &mut egui::Ui, t: &str| {
                                            ui.label(egui::RichText::new(t).strong());
                                        };
                                        lbl(ui, self.t(i18n::AUTEURS));
                                        ui.label("Roman Czyborra, Paul Hardy & contributeurs");
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::ROLE));
                                        ui.add(
                                            egui::Label::new(unifont_role)
                                                .wrap_mode(egui::TextWrapMode::Wrap),
                                        );
                                        ui.end_row();
                                        lbl(ui, self.t(i18n::LICENCE));
                                        ui.label("SIL Open Font License v1.1 / GPLv2+ avec exception d'embarquement");
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
        self.apply_storage_actions(storage_actions);
    }

    /// Applique les demandes de l'onglet Stockage. Hors de la closure de la
    /// fenêtre : chacune reprend le verrou de l'état partagé.
    fn apply_storage_actions(&mut self, actions: StorageActions) {
        // Durée d'abord : la simulation qui suit doit porter sur elle.
        if actions.save {
            let days = self.retention_days.to_string();
            let images = if self.purge_includes_images { "1" } else { "0" };
            let mut state = self.state.lock_safe();
            state.set_pref("media_retention_days", &days);
            state.set_pref("media_purge_images", images);
            drop(state);
            self.purge_preview = None;
        }
        if actions.open_folder {
            let dir = self.state.lock_safe().media_dir().to_path_buf();
            if let Err(error) = open_in_file_manager(&dir) {
                tracing::warn!("ouverture du dossier des médias : {error}");
                self.notify(self.t(i18n::OUVERTURE_DU_DOSSIER_IMPOSSIBLE));
            }
        }
        if actions.refresh && !self.storage_scan_pending {
            self.storage_scan_pending = true;
            self.state.lock_safe().request_storage_usage();
        }
        // Une purge réelle rend son propre compte rendu : inutile de simuler
        // en plus, et la simulation d'après portera sur le dossier nettoyé.
        if actions.purge {
            self.purge_preview = None;
            self.state.lock_safe().request_media_gc(false);
            return;
        }
        if actions.preview {
            self.purge_preview = None;
        }
        // L'aperçu se redemande tant qu'il manque : c'est lui qui décide si le
        // bouton « Purger » est actif.
        let on_storage_tab =
            self.modals.settings_open && self.modals.settings_tab == SettingsTab::Storage;
        if on_storage_tab && self.purge_preview.is_none() && !self.purge_preview_pending {
            self.purge_preview_pending = true;
            self.state.lock_safe().request_media_gc(true);
        }
    }
}

/// Ouvre un dossier dans l'explorateur de fichiers du système.
///
/// C'est le filet de sécurité de la gestion du stockage : plus rien n'est
/// supprimé automatiquement, alors l'utilisateur doit pouvoir aller faire le
/// ménage lui-même. Les pièces jointes sont nommées `<horodatage>-<nom>`, donc
/// un tri par nom dans l'explorateur les range par date.
fn open_in_file_manager(dir: &std::path::Path) -> std::io::Result<()> {
    // Le dossier n'existe pas tant qu'aucun média n'est passé : le créer évite
    // un échec incompréhensible sur une installation neuve.
    std::fs::create_dir_all(dir)?;
    let command = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    // `explorer` rend un code non nul même quand il a ouvert la fenêtre : on
    // se contente de savoir que le processus a démarré.
    std::process::Command::new(command).arg(dir).spawn()?;
    Ok(())
}

/// Formate une taille en unité lisible. Les octets bruts d'un cache de
/// plusieurs gigaoctets ne disent rien à l'œil.
pub(crate) fn human_bytes(bytes: u64) -> String {
    const UNITS: [(&str, u64); 3] = [("Go", 1_000_000_000), ("Mo", 1_000_000), ("ko", 1_000)];
    for (unit, scale) in UNITS {
        if bytes >= scale {
            return format!("{:.1} {unit}", bytes as f64 / scale as f64);
        }
    }
    format!("{bytes} o")
}

/// Ce que l'onglet Stockage demande à l'application, appliqué hors de la
/// closure de la fenêtre pour éviter tout emprunt concurrent de `self`.
#[derive(Default)]
struct StorageActions {
    /// Recalculer la ventilation disque.
    refresh: bool,
    /// Recalculer l'aperçu (simulation de purge).
    preview: bool,
    /// Purger réellement.
    purge: bool,
    /// Enregistrer la durée demandée.
    save: bool,
    /// Ouvrir le dossier des pièces jointes dans l'explorateur du système.
    open_folder: bool,
}

/// Dix ans : au-delà, « conserver » et « tout garder » se confondent.
const RETENTION_DAYS_MAX: u32 = 3650;

/// Onglet Stockage : ventilation de l'occupation disque et règles de
/// conservation des pièces jointes.
fn storage_tab(
    ui: &mut egui::Ui,
    usage: Option<&crate::app::usage::Usage>,
    preview: Option<&crate::app::media::GcReport>,
    retention_days: &mut u32,
    include_images: &mut bool,
    language: UiLanguage,
    actions: &mut StorageActions,
) {
    let Some(usage) = usage else {
        // Premier affichage : le calcul n'a pas encore répondu.
        actions.refresh = true;
        ui.label(egui::RichText::new(i18n::CALCUL_EN_COURS.get(language)).weak());
        return;
    };

    let total = usage.total();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(i18n::ESPACE_OCCUPE.get(language)).strong());
        ui.label(
            egui::RichText::new(human_bytes(total.bytes))
                .heading()
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(i18n::RECALCULER.get(language)).clicked() {
                actions.refresh = true;
                actions.preview = true;
            }
        });
    });
    ui.label(
        egui::RichText::new(format!(
            "{} {}",
            total.files,
            i18n::UNITE_FICHIERS.get(language)
        ))
        .small()
        .weak(),
    );
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);

    egui::Grid::new("storage_breakdown")
        .num_columns(3)
        .spacing([16.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            // Les envois d'abord parmi les médias : ce sont des copies dont
            // l'original est ailleurs, donc le poste le plus sûr à purger.
            let rows = [
                (i18n::MEDIAS_RECUS.get(language), usage.media_received),
                (i18n::MEDIAS_ENVOYES.get(language), usage.media_sent),
                (i18n::TRANSFERTS_INACHEVES.get(language), usage.incomplete),
                (i18n::HISTORIQUE.get(language), usage.database),
                (i18n::IMAGE_DE_PROFIL.get(language), usage.avatar),
                (i18n::JOURNAUX.get(language), usage.logs),
                (i18n::FICHIERS_DE_TRAVAIL.get(language), usage.scratch),
                (i18n::AUTRES.get(language), usage.other),
            ];
            for (label, entry) in rows {
                ui.label(label);
                ui.label(human_bytes(entry.bytes));
                let detail = if entry.files == 0 {
                    String::new()
                } else {
                    format!("{} {}", entry.files, i18n::UNITE_FICHIERS.get(language))
                };
                ui.label(egui::RichText::new(detail).small().weak());
                ui.end_row();
            }
        });

    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(i18n::LES_ENVOIS_SONT_DES_COPIES.get(language))
            .small()
            .weak(),
    );

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);
    ui.label(egui::RichText::new(i18n::PURGE_MANUELLE.get(language)).strong());
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(i18n::AUCUNE_PURGE_AUTOMATIQUE.get(language))
            .small()
            .weak(),
    );
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(i18n::SUPPRIMER_LES_PIECES_JOINTES_AU_DELA.get(language));
        if ui
            .add(
                egui::DragValue::new(retention_days)
                    .speed(1.0)
                    .range(0..=RETENTION_DAYS_MAX),
            )
            .changed()
        {
            actions.save = true;
        }
        ui.label(i18n::UNITE_JOURS.get(language));
        ui.label(
            egui::RichText::new(i18n::ZERO_DECHETS_SEULEMENT.get(language))
                .small()
                .weak(),
        );
    });

    ui.add_space(6.0);
    if ui
        .checkbox(include_images, i18n::INCLURE_LES_IMAGES.get(language))
        .on_hover_text(i18n::INCLURE_LES_IMAGES_AIDE.get(language))
        .changed()
    {
        actions.save = true;
    }

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let freed = preview.map(|report| report.freed_bytes).unwrap_or(0);
        if ui
            .add_enabled(
                freed > 0,
                egui::Button::new(i18n::PURGER_MAINTENANT.get(language)),
            )
            .clicked()
        {
            actions.purge = true;
        }
        if ui
            .button(i18n::OUVRIR_LE_DOSSIER.get(language))
            .on_hover_text(i18n::POUR_SUPPRIMER_A_LA_MAIN.get(language))
            .clicked()
        {
            actions.open_folder = true;
        }
        let hint = match preview {
            None => i18n::CALCUL_EN_COURS.get(language).to_string(),
            Some(report) if report.freed_files == 0 => {
                i18n::RIEN_A_PURGER.get(language).to_string()
            }
            Some(report) => i18n::LIBERERAIT_MODELE
                .get(language)
                .replace("{taille}", &human_bytes(report.freed_bytes))
                .replace("{fichiers}", &report.freed_files.to_string()),
        };
        ui.label(egui::RichText::new(hint).small().weak());
    });
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(i18n::LE_MESSAGE_RESTE_DANS_LE_FIL.get(language))
            .small()
            .weak(),
    );
}
