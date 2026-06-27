use chrono::{Datelike, Local, NaiveDate, TimeZone};
use eframe::egui;

use crate::app::AppState;
use crate::message::ChatMessage;
use crate::transfer::{TransferDecision, TransferDirection, TransferStatus};

use super::{AbcomApp, UiLanguage};

/// Diamètre de l'avatar affiché en tête de chaque groupe de messages.
const AVATAR_SIZE: f32 = 40.0;
/// Espace horizontal entre l'avatar et le texte du message.
const AVATAR_GUTTER: f32 = 12.0;
/// Espace vertical séparant deux groupes de messages.
const GROUP_SPACING: f32 = 10.0;
/// Écart de temps au-delà duquel un nouvel en-tête est ouvert pour un même
/// auteur (façon Discord/Cinny). Évite que des messages espacés de plusieurs
/// heures ou jours paraissent envoyés d'un coup. Ajustable.
const GROUP_BREAK_SECS: u64 = 5 * 60;
/// Couleur du nom pour nos propres messages (conservée partout).
const OWN_NAME_COLOR: egui::Color32 = egui::Color32::from_rgb(80, 200, 120);
/// Couleur du nom d'un autre pair en conversation 1-à-1.
const PEER_NAME_COLOR: egui::Color32 = egui::Color32::from_rgb(100, 180, 255);

/// Palette de couleurs distinctes pour les pairs dans les vues multi-personnes
/// (groupes et « Tous »), inspirée des couleurs d'utilisateur de Cinny.
const PEER_PALETTE: [egui::Color32; 8] = [
    egui::Color32::from_rgb(128, 195, 255),
    egui::Color32::from_rgb(255, 153, 253),
    egui::Color32::from_rgb(102, 255, 212),
    egui::Color32::from_rgb(255, 128, 164),
    egui::Color32::from_rgb(255, 163, 102),
    egui::Color32::from_rgb(51, 252, 255),
    egui::Color32::from_rgb(158, 153, 255),
    egui::Color32::from_rgb(197, 255, 153),
];

/// Couleur déterministe attribuée à un pair (même nom → même couleur) pour le
/// distinguer des autres participants dans une conversation multi-personnes.
fn peer_color(username: &str) -> egui::Color32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    username.hash(&mut hasher);
    PEER_PALETTE[(hasher.finish() as usize) % PEER_PALETTE.len()]
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
fn message_day(msg: &ChatMessage) -> Option<NaiveDate> {
    let epoch = msg.timestamp_epoch?;
    Local
        .timestamp_opt(epoch as i64, 0)
        .single()
        .map(|dt| dt.date_naive())
}

/// Heure d'en-tête au format 24 h, dérivée de l'instant Unix si présent,
/// sinon repli sur la chaîne `timestamp` (anciens messages / pairs).
fn header_time(msg: &ChatMessage) -> String {
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
fn starts_new_group(
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
fn day_divider_label(date: NaiveDate, today: NaiveDate, language: UiLanguage) -> String {
    if date == today {
        return match language {
            UiLanguage::French => "Aujourd'hui".to_string(),
            UiLanguage::English => "Today".to_string(),
        };
    }
    if Some(date) == today.pred_opt() {
        return match language {
            UiLanguage::French => "Hier".to_string(),
            UiLanguage::English => "Yesterday".to_string(),
        };
    }
    let (day, month, year) = (date.day(), date.month0() as usize, date.year());
    match language {
        UiLanguage::French => format!("{} {} {}", day, MONTHS_FR[month], year),
        UiLanguage::English => format!("{} {}, {}", MONTHS_EN[month], day, year),
    }
}

/// Dessine un séparateur de date pleine largeur : une ligne fine traversée par
/// le libellé centré, façon Discord/Cinny.
fn render_day_divider(ui: &mut egui::Ui, label: &str) {
    ui.add_space(14.0);
    let line_color = egui::Color32::from_gray(80);
    let text_color = egui::Color32::from_gray(150);
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
/// l'heure d'envoi (format 24 h) et, pour nos messages, de l'accusé de lecture.
fn render_message_header(
    ui: &mut egui::Ui,
    display_name: &str,
    timestamp: &str,
    name_color: egui::Color32,
    receipt: Option<(bool, bool)>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            egui::RichText::new(display_name)
                .color(name_color)
                .family(egui::FontFamily::Name(super::BOLD_FAMILY.into())),
        );
        ui.label(
            egui::RichText::new(timestamp)
                .small()
                .color(egui::Color32::from_gray(140)),
        );
        if let Some((delivered, read)) = receipt {
            show_receipt(ui, delivered, read);
        }
    });
}

impl AbcomApp {
    /// Zone centrale : fil de la conversation sélectionnée
    pub(crate) fn show_central_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (selected_conv, my_name, conv_messages) = {
                let s = self.state.lock().unwrap();
                let selected = s.selected_conversation.clone();
                let my_username = s.my_username.clone();
                let msgs: Vec<_> = s.get_conversation_messages().into_iter().cloned().collect();
                (selected, my_username, msgs)
            };

            // Conversation privée (1-à-1) = pair sélectionné qui n'est pas un groupe `#…`
            let private_peer = selected_conv
                .as_deref()
                .filter(|c| !c.starts_with('#'))
                .map(str::to_string);
            let is_broadcast = selected_conv.is_none();

            let conversation_title = match &private_peer {
                Some(user) => self.state.lock().unwrap().peer_display_name(user),
                None => selected_conv
                    .clone()
                    .unwrap_or_else(|| self.tr("Tous", "All").to_string()),
            };

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.heading(&conversation_title);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button(self.tr("Actions", "Actions"), |ui| {
                        let sound_text = if self.enable_sound_notifications {
                            self.tr("🔊 Désactiver tous les sons", "🔊 Disable all sounds")
                        } else {
                            self.tr("🔇 Activer tous les sons", "🔇 Enable all sounds")
                        };
                        if ui.button(sound_text).clicked() {
                            self.enable_sound_notifications = !self.enable_sound_notifications;
                            ui.close_menu();
                        }
                        let this_conv = selected_conv.clone();
                        let is_muted = self.muted_conversations.contains(&this_conv);
                        let mute_text = if is_muted {
                            self.tr(
                                "🔔 Réactiver les sons de ce salon",
                                "🔔 Re-enable sounds for this chat",
                            )
                        } else {
                            self.tr("🔕 Muet pour ce salon", "🔕 Mute this chat")
                        };
                        if ui.button(mute_text).clicked() {
                            if is_muted {
                                self.muted_conversations.remove(&this_conv);
                            } else {
                                self.muted_conversations.insert(this_conv);
                            }
                            ui.close_menu();
                        }
                        if ui
                            .button(self.tr("👥 Voir les participants", "👥 View participants"))
                            .clicked()
                        {
                            self.show_participants = true;
                            ui.close_menu();
                        }
                        if let Some(user) = &private_peer {
                            if ui
                                .button(self.tr("Renommer ce contact", "Rename contact"))
                                .clicked()
                            {
                                self.rename_input = self
                                    .state
                                    .lock()
                                    .unwrap()
                                    .peer_records
                                    .iter()
                                    .find(|r| &r.username == user)
                                    .and_then(|r| r.alias.clone())
                                    .unwrap_or_default();
                                self.rename_target = Some(user.clone());
                                ui.close_menu();
                            }
                        }
                        if !is_broadcast
                            && ui
                                .button(self.tr("🗑 Effacer l'historique", "🗑 Clear history"))
                                .clicked()
                        {
                            self.state.lock().unwrap().clear_conversation_history();
                            ui.close_menu();
                        }
                    });
                });
            });
            ui.separator();

            // Popup participants
            if self.show_participants {
                let (conv_name, my_name2, sel_conv, peers) = {
                    let s = self.state.lock().unwrap();
                    (
                        s.selected_conversation
                            .clone()
                            .unwrap_or_else(|| self.tr("Tous", "All").to_string()),
                        s.my_username.clone(),
                        s.selected_conversation.clone(),
                        s.peers.clone(),
                    )
                };
                let mut open = self.show_participants;
                egui::Window::new(self.tr("Participants", "Participants"))
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {}",
                                self.tr("Conversation", "Conversation"),
                                conv_name
                            ))
                            .strong(),
                        );
                        ui.separator();
                        if sel_conv.is_none() {
                            for peer in &peers {
                                ui.label(&peer.username);
                            }
                            if peers.is_empty() {
                                ui.label(
                                    self.tr(
                                        "Aucun participant connecté",
                                        "No connected participant",
                                    ),
                                );
                            }
                        } else {
                            ui.label(format!("{} ({})", my_name2, self.tr("vous", "you")));
                            if let Some(peer) = sel_conv {
                                ui.label(&peer);
                            }
                        }
                    });
                self.show_participants = open;
            }

            // Modale de renommage de contact
            if let Some(target) = self.rename_target.clone() {
                // Libellés calculés avant la closure (évite d'emprunter `self`
                // pendant qu'on édite `self.rename_input`).
                let title = self.tr("Renommer le contact", "Rename contact");
                let lbl_original = self.tr("Nom d'origine", "Original name");
                let hint = self.tr("Alias (vide = retirer)", "Alias (empty = remove)");
                let save_lbl = self.tr("Enregistrer", "Save");
                let clear_lbl = self.tr("Retirer l'alias", "Remove alias");

                let mut open = true;
                let mut do_save = false;
                let mut do_clear = false;
                egui::Window::new(title)
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(format!("{}: {}", lbl_original, target));
                        ui.add_space(6.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.rename_input)
                                .hint_text(hint)
                                .desired_width(240.0),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            do_save = ui.button(save_lbl).clicked();
                            do_clear = ui.button(clear_lbl).clicked();
                        });
                    });

                if do_save {
                    let trimmed = self.rename_input.trim();
                    let alias = (!trimmed.is_empty()).then(|| trimmed.to_string());
                    self.state.lock().unwrap().set_peer_alias(&target, alias);
                    self.rename_target = None;
                } else if do_clear {
                    self.state.lock().unwrap().set_peer_alias(&target, None);
                    self.rename_target = None;
                } else if !open {
                    self.rename_target = None;
                }
            }

            // Avatars et noms d'affichage des auteurs, préparés avant la zone
            // défilante (le chargement de texture emprunte `self` mutablement).
            let unique_authors: Vec<String> = {
                let mut authors: Vec<String> =
                    conv_messages.iter().map(|m| m.from.clone()).collect();
                authors.sort();
                authors.dedup();
                authors
            };
            let mut author_avatars: std::collections::HashMap<String, Option<egui::TextureHandle>> =
                std::collections::HashMap::new();
            for author in &unique_authors {
                let texture = self.avatar_texture(ctx, author);
                author_avatars.insert(author.clone(), texture);
            }
            let author_names: std::collections::HashMap<String, String> = {
                let s = self.state.lock().unwrap();
                unique_authors
                    .iter()
                    .map(|a| (a.clone(), s.peer_display_name(a)))
                    .collect()
            };

            // Aire de messages
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if conv_messages.is_empty() {
                        ui.add_space(50.0);
                        ui.label(
                            egui::RichText::new(self.tr("Aucun message", "No message")).weak(),
                        );
                    }

                    // Rendu façon Discord : les messages consécutifs d'un même
                    // auteur sont regroupés sous un seul en-tête (avatar + nom +
                    // heure), les suivants étant alignés sous le texte. Un écart
                    // de temps ou un changement de jour rouvre un en-tête, et un
                    // séparateur de date marque chaque nouvelle journée.
                    let language = self.ui_language;
                    let today = Local::now().date_naive();
                    // Vue multi-personnes (groupe `#…` ou « Tous ») : chaque pair
                    // reçoit une couleur distincte ; en 1-à-1 on garde le bleu.
                    let multi_person = selected_conv.as_deref().is_none_or(|c| c.starts_with('#'));
                    let mut last_from: Option<&str> = None;
                    let mut last_epoch: Option<u64> = None;
                    let mut last_day: Option<NaiveDate> = None;
                    for msg in &conv_messages {
                        let day = message_day(msg);
                        let day_changed = match (day, last_day) {
                            (Some(d), Some(prev)) => d != prev,
                            (Some(_), None) => last_from.is_some(),
                            _ => false,
                        };
                        if let Some(d) = day {
                            if day_changed || last_day.is_none() {
                                render_day_divider(ui, &day_divider_label(d, today, language));
                            }
                        }

                        let starts_group = starts_new_group(
                            last_from,
                            last_epoch,
                            &msg.from,
                            msg.timestamp_epoch,
                            day_changed,
                        );
                        let is_me = msg.from == my_name;
                        let name_color = if is_me {
                            OWN_NAME_COLOR
                        } else if multi_person {
                            peer_color(&msg.from)
                        } else {
                            PEER_NAME_COLOR
                        };
                        // Accusé de réception de nos messages : ✓ envoyé,
                        // ✓✓ gris livré (ACK), ✓✓ bleu lu (ReadReceipt).
                        let receipt = is_me.then(|| {
                            let hash = AppState::message_hash(msg);
                            let s = self.state.lock().unwrap();
                            (!s.is_message_pending(hash), s.get_read_count(hash) > 0)
                        });

                        if starts_group {
                            ui.add_space(GROUP_SPACING);
                            ui.horizontal(|ui| {
                                // Retrait du texte = avatar + gouttière, sans
                                // espacement parasite, pour qu'il coïncide avec
                                // les messages de continuation (cf. branche else).
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let avatar = author_avatars
                                    .get(&msg.from)
                                    .and_then(|texture| texture.as_ref());
                                let display = author_names
                                    .get(&msg.from)
                                    .map(String::as_str)
                                    .unwrap_or(&msg.from);
                                super::avatar::show_avatar(ui, avatar, display, AVATAR_SIZE);
                                ui.add_space(AVATAR_GUTTER);
                                ui.vertical(|ui| {
                                    render_message_header(
                                        ui,
                                        display,
                                        &header_time(msg),
                                        name_color,
                                        receipt,
                                    );
                                    super::markdown::render_message_markdown(
                                        ui,
                                        &msg.content,
                                        &self.emoji_map,
                                        &self.emoji_textures,
                                    );
                                });
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.add_space(AVATAR_SIZE + AVATAR_GUTTER);
                                ui.vertical(|ui| {
                                    super::markdown::render_message_markdown(
                                        ui,
                                        &msg.content,
                                        &self.emoji_map,
                                        &self.emoji_textures,
                                    );
                                });
                            });
                        }
                        last_from = Some(msg.from.as_str());
                        last_epoch = msg.timestamp_epoch;
                        if day.is_some() {
                            last_day = day;
                        }
                    }

                    // Transferts et propositions de fichiers, intégrés au fil
                    self.render_transfer_cards(ui, &selected_conv);
                });
        });
    }

    /// Rend, dans le fil de la conversation, les propositions de réception en
    /// attente puis la progression des transferts liés à cette conversation.
    /// Affichées en vue globale (« Tous ») ou dans la conversation du pair.
    fn render_transfer_cards(&mut self, ui: &mut egui::Ui, selected_conv: &Option<String>) {
        // ── Propositions de réception (Accepter / Refuser) ──────────────────
        let offers: Vec<(String, String, String, u64, usize)> = self
            .pending_offers
            .iter()
            .filter(|o| {
                selected_conv.is_none() || selected_conv.as_deref() == Some(o.from.as_str())
            })
            .map(|o| {
                (
                    o.transfer_id.clone(),
                    o.from.clone(),
                    o.label.clone(),
                    o.total_bytes,
                    o.item_count,
                )
            })
            .collect();

        let mut accept_id: Option<String> = None;
        let mut refuse_id: Option<String> = None;
        for (transfer_id, from, label, total, count) in &offers {
            ui.add_space(6.0);
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(48, 52, 60))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            from,
                            self.tr("vous envoie un fichier", "is sending you a file")
                        ))
                        .strong(),
                    );
                    let detail = if *count > 1 {
                        format!(
                            "{} ({}, {} {})",
                            label,
                            format_bytes(*total),
                            count,
                            self.tr("éléments", "items")
                        )
                    } else {
                        format!("{} ({})", label, format_bytes(*total))
                    };
                    ui.label(egui::RichText::new(detail).small());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(self.tr("Refuser", "Decline")).clicked() {
                            refuse_id = Some(transfer_id.clone());
                        }
                        if ui.button(self.tr("Accepter", "Accept")).clicked() {
                            accept_id = Some(transfer_id.clone());
                        }
                    });
                });
        }
        if let Some(id) = accept_id {
            // Le choix du dossier est différé d'une frame (conflit AppKit macOS).
            self.pending_accept = Some(id);
        }
        if let Some(id) = refuse_id {
            if let Some(pos) = self.pending_offers.iter().position(|o| o.transfer_id == id) {
                let offer = self.pending_offers.remove(pos);
                let _ = offer.decision_tx.send(TransferDecision {
                    accept: false,
                    dest_dir: None,
                });
            }
        }

        // ── Progression des transferts ──────────────────────────────────────
        let mut transfers: Vec<_> = self
            .transfer_progress
            .values()
            .filter(|t| !self.dismissed_transfers.contains(&t.transfer_id))
            .filter(|t| {
                selected_conv.is_none() || selected_conv.as_deref() == Some(t.peer.as_str())
            })
            .cloned()
            .collect();
        transfers.sort_by(|a, b| a.transfer_id.cmp(&b.transfer_id));

        let mut dismiss_id: Option<String> = None;
        for t in &transfers {
            ui.add_space(6.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let dir = match t.direction {
                        TransferDirection::Upload => self.tr("Envoi", "Sent"),
                        TransferDirection::Download => self.tr("Réception", "Received"),
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {} ({})",
                            dir,
                            t.label,
                            format_bytes(t.total_bytes)
                        ))
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if close_button(ui) {
                            dismiss_id = Some(t.transfer_id.clone());
                        }
                        let status = match t.status {
                            TransferStatus::Queued => self.tr("En attente", "Queued"),
                            TransferStatus::Running => self.tr("En cours", "Running"),
                            TransferStatus::Completed => self.tr("Terminé", "Done"),
                            TransferStatus::Failed => self.tr("Échec", "Failed"),
                            TransferStatus::Rejected => self.tr("Refusé", "Declined"),
                        };
                        ui.label(egui::RichText::new(status).small());
                    });
                });
                if t.status == TransferStatus::Running && t.total_bytes > 0 {
                    let ratio = (t.bytes_done as f32 / t.total_bytes as f32).clamp(0.0, 1.0);
                    ui.add(
                        egui::ProgressBar::new(ratio)
                            .show_percentage()
                            .desired_width(ui.available_width()),
                    );
                    if let Some(path) = &t.current_path {
                        ui.label(
                            egui::RichText::new(path)
                                .small()
                                .color(egui::Color32::from_gray(150)),
                        );
                    }
                }
                if !t.detail.is_empty() {
                    ui.label(
                        egui::RichText::new(&t.detail)
                            .small()
                            .color(egui::Color32::from_gray(160)),
                    );
                }
            });
        }
        if let Some(id) = dismiss_id {
            self.dismissed_transfers.insert(id);
        }
    }
}

/// Formate une taille en octets de façon lisible (o / Ko / Mo / Go).
fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} Go", b / GB)
    } else if b >= MB {
        format!("{:.1} Mo", b / MB)
    } else if b >= KB {
        format!("{:.1} Ko", b / KB)
    } else {
        format!("{} o", bytes)
    }
}

/// Petite croix de fermeture peinte (pas de glyphe, rendu fiable). Renvoie `true` au clic.
fn close_button(ui: &mut egui::Ui) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() {
            egui::Color32::from_rgb(230, 120, 120)
        } else {
            egui::Color32::from_gray(150)
        };
        let stroke = egui::Stroke::new(1.5, color);
        let p = ui.painter();
        let c = rect.center();
        let d = 3.5;
        p.line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], stroke);
        p.line_segment([c + egui::vec2(d, -d), c + egui::vec2(-d, d)], stroke);
    }
    resp.clicked()
}

/// Dessine une ou deux coches selon le statut de lecture du message.
/// `read = false` → une coche grise (envoyé) ; `read = true` → deux coches bleues (lu).
/// Affiche l'indicateur de statut d'un message :
/// - ✓  gris  = envoyé, livraison en attente
/// - ✓✓ gris  = livré (ACK reçu), pas encore lu
/// - ✓✓ bleu  = lu (ReadReceipt reçu)
fn show_receipt(ui: &mut egui::Ui, delivered: bool, read: bool) {
    let double = delivered || read;
    let w = if double { 17.0_f32 } else { 9.0_f32 };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 12.0), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let color = if read {
        egui::Color32::from_rgb(80, 180, 255) // bleu = lu
    } else {
        egui::Color32::from_gray(160) // gris = envoyé ou livré
    };
    let stroke = egui::Stroke::new(1.5, color);
    let p = ui.painter();

    let draw_tick = |ox: f32| {
        let base = rect.left_top() + egui::vec2(ox, 4.0);
        p.line_segment([base, base + egui::vec2(2.5, 3.0)], stroke);
        p.line_segment(
            [base + egui::vec2(2.5, 3.0), base + egui::vec2(8.0, -1.5)],
            stroke,
        );
    };

    draw_tick(0.0);
    if double {
        draw_tick(6.0);
    }
}

#[cfg(test)]
mod tests {
    use super::{day_divider_label, starts_new_group, GROUP_BREAK_SECS};
    use crate::ui::UiLanguage;
    use chrono::NaiveDate;

    #[test]
    fn group_breaks_on_author_change() {
        assert!(starts_new_group(
            Some("alice"),
            Some(100),
            "bob",
            Some(110),
            false
        ));
    }

    #[test]
    fn group_breaks_on_day_change() {
        assert!(starts_new_group(
            Some("alice"),
            Some(100),
            "alice",
            Some(110),
            true
        ));
    }

    #[test]
    fn group_keeps_same_author_within_window() {
        assert!(!starts_new_group(
            Some("alice"),
            Some(1_000),
            "alice",
            Some(1_000 + GROUP_BREAK_SECS),
            false,
        ));
    }

    #[test]
    fn group_breaks_after_time_gap() {
        assert!(starts_new_group(
            Some("alice"),
            Some(1_000),
            "alice",
            Some(1_000 + GROUP_BREAK_SECS + 1),
            false,
        ));
    }

    #[test]
    fn group_falls_back_to_author_without_epoch() {
        // Sans instants comparables : même auteur reste groupé.
        assert!(!starts_new_group(Some("alice"), None, "alice", None, false));
    }

    #[test]
    fn divider_labels_today_and_yesterday() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
        let yesterday = NaiveDate::from_ymd_opt(2026, 5, 19).unwrap();
        assert_eq!(
            day_divider_label(today, today, UiLanguage::French),
            "Aujourd'hui"
        );
        assert_eq!(
            day_divider_label(today, today, UiLanguage::English),
            "Today"
        );
        assert_eq!(
            day_divider_label(yesterday, today, UiLanguage::French),
            "Hier"
        );
        assert_eq!(
            day_divider_label(yesterday, today, UiLanguage::English),
            "Yesterday"
        );
    }

    #[test]
    fn divider_labels_full_date_localized() {
        let today = NaiveDate::from_ymd_opt(2026, 6, 23).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 5, 18).unwrap();
        assert_eq!(
            day_divider_label(date, today, UiLanguage::French),
            "18 mai 2026"
        );
        assert_eq!(
            day_divider_label(date, today, UiLanguage::English),
            "May 18, 2026"
        );
    }
}
