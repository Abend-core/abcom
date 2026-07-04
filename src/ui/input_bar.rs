use eframe::egui;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use std::sync::{Arc, Mutex};

use crate::app::AppState;
use crate::message::{
    ChatMessage, MediaAttachment, MediaKind, MediaSendJob, MediaStreamHeader, SendRequest,
    TypingIndicator, TypingRequest,
};

use super::composer;
use super::emoji_picker::emoji_shortcode_trigger;
use super::AbcomApp;

const ACTION_BUTTON_SIZE: [f32; 2] = [34.0, 34.0];

/// Au-delà de cette taille (1 Go), l'envoi d'un média demande l'accord du
/// destinataire avant transfert. En dessous, l'envoi est automatique. Dans les
/// deux cas, c'est le même chemin (streaming par morceaux).
const MEDIA_ACK_THRESHOLD: u64 = 1024 * 1024 * 1024;

enum AttachmentMenuAction {
    AddFiles,
    AddFolder,
}

fn should_send_message(
    pressed_enter: bool,
    pressed_enter_fallback: bool,
    shortcode_menu_open: bool,
    input: &str,
) -> bool {
    (pressed_enter || (pressed_enter_fallback && !shortcode_menu_open)) && !input.trim().is_empty()
}

#[cfg(test)]
fn push_unique_paths(target: &mut Vec<PathBuf>, paths: impl IntoIterator<Item = PathBuf>) {
    for path in paths {
        if !target.iter().any(|existing| existing == &path) {
            target.push(path);
        }
    }
}

fn attachment_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn action_button_chrome(selected: bool) -> (egui::Color32, egui::Stroke) {
    let fill = if selected {
        egui::Color32::from_rgb(88, 122, 255)
    } else {
        egui::Color32::from_rgb(78, 78, 82)
    };
    let stroke = if selected {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(132, 158, 255))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(104, 104, 108))
    };
    (fill, stroke)
}

fn action_button(
    ui: &mut egui::Ui,
    label: egui::RichText,
    tooltip: &str,
    selected: bool,
) -> egui::Response {
    let (fill, stroke) = action_button_chrome(selected);
    ui.add_sized(
        ACTION_BUTTON_SIZE,
        egui::Button::new(label)
            .fill(fill)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(10)),
    )
    .on_hover_text(tooltip)
}

fn icon_button(
    ui: &mut egui::Ui,
    tooltip: &str,
    selected: bool,
    paint: impl FnOnce(&egui::Painter, egui::Rect, egui::Color32),
) -> egui::Response {
    let (fill, stroke) = action_button_chrome(selected);
    let response = ui
        .add_sized(
            ACTION_BUTTON_SIZE,
            egui::Button::new(egui::RichText::new(""))
                .fill(fill)
                .stroke(stroke)
                .corner_radius(egui::CornerRadius::same(10)),
        )
        .on_hover_text(tooltip);
    paint(
        ui.painter(),
        response.rect.shrink2(egui::vec2(8.0, 8.0)),
        egui::Color32::from_rgb(244, 245, 247),
    );
    response
}

/// Petite croix peinte pour retirer une pièce jointe (glyphe « ✕ » non rendu de
/// façon fiable par la police). Renvoie `true` au clic.
fn chip_remove_button(ui: &mut egui::Ui) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() {
            egui::Color32::from_rgb(235, 120, 120)
        } else {
            egui::Color32::from_gray(200)
        };
        let stroke = egui::Stroke::new(1.6, color);
        let c = rect.center();
        let d = 3.5;
        let p = ui.painter();
        p.line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], stroke);
        p.line_segment([c + egui::vec2(d, -d), c + egui::vec2(-d, d)], stroke);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn paint_plus_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
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

fn paint_send_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
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

fn attachment_menu_popup(
    ctx: &egui::Context,
    anchor_rect: egui::Rect,
    add_files_label: &str,
    add_folder_label: &str,
) -> Option<AttachmentMenuAction> {
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

    area.show(ctx, |ui| {
        egui::Frame::popup(ui.style())
            .fill(egui::Color32::from_rgb(58, 58, 62))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(102, 102, 108),
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
    })
    .inner
}

/// Envoie un fichier (ou un dossier zippé) comme média, par streaming. Tout le
/// travail lourd (zip, copie locale dans `media/<id>`, lecture) se fait dans un
/// thread dédié pour ne jamais geler l'UI, même pour plusieurs Go.
fn send_one_media(
    app: &AbcomApp,
    path: &Path,
    my_name: &str,
    to_user: &Option<String>,
    targets: &[(String, std::net::SocketAddr)],
) {
    let state = app.state.clone();
    let send_media_tx = app.send_media_tx.clone();
    let path = path.to_path_buf();
    let my_name = my_name.to_string();
    let to_user = to_user.clone();
    let targets = targets.to_vec();

    std::thread::spawn(move || {
        if let Err(e) =
            prepare_and_stream(&state, &send_media_tx, &path, &my_name, &to_user, &targets)
        {
            eprintln!("[ui] préparation média échouée ({}): {}", path.display(), e);
        }
    });
}

/// Prépare un média dans `media/<id>` (copie d'un fichier ou zip d'un dossier),
/// l'ajoute à notre historique, puis met en file un envoi vers chaque pair.
fn prepare_and_stream(
    state: &Arc<Mutex<AppState>>,
    send_media_tx: &tokio::sync::mpsc::Sender<MediaSendJob>,
    path: &Path,
    my_name: &str,
    to_user: &Option<String>,
    targets: &[(String, std::net::SocketAddr)],
) -> std::io::Result<()> {
    let is_dir = path.is_dir();
    let filename = super::media::media_display_name(path);
    let id = super::media::media_id(&filename);

    let dest = state.lock().unwrap().media_path(&id);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if is_dir {
        crate::archive::zip_dir_to_path(path, &dest)?;
    } else {
        std::fs::copy(path, &dest)?;
    }

    let size_bytes = std::fs::metadata(&dest)?.len();
    let (kind, width, height) = if !is_dir && MediaAttachment::is_image_filename(&filename) {
        let dims = image::image_dimensions(&dest).ok();
        (MediaKind::Image, dims.map(|d| d.0), dims.map(|d| d.1))
    } else {
        (MediaKind::File, None, None)
    };

    let media = MediaAttachment {
        id,
        filename,
        kind,
        size_bytes,
        url: None,
        width,
        height,
    };
    let now = chrono::Local::now();
    let header = MediaStreamHeader {
        from: my_name.to_string(),
        to_user: to_user.clone(),
        timestamp: now.format("%H:%M").to_string(),
        timestamp_epoch: Some(now.timestamp() as u64),
        media: media.clone(),
        requires_ack: size_bytes > MEDIA_ACK_THRESHOLD,
    };

    // Notre propre copie du message (la carte apparaît, avec progression).
    state.lock().unwrap().add_message(ChatMessage {
        from: my_name.to_string(),
        content: String::new(),
        timestamp: header.timestamp.clone(),
        timestamp_epoch: header.timestamp_epoch,
        to_user: to_user.clone(),
        media: Some(media),
        reply_to: None,
    });

    for (_, addr) in targets {
        let _ = send_media_tx.try_send(MediaSendJob {
            to_addr: *addr,
            source_path: dest.clone(),
            header: header.clone(),
        });
    }
    Ok(())
}

fn send_current_message(
    app: &mut AbcomApp,
    selected_addr: Option<std::net::SocketAddr>,
    all_peers: &[crate::app::Peer],
) -> bool {
    let has_message = !app.input.trim().is_empty();
    let has_attachments = !app.pending_attachments.is_empty();
    if !has_message && !has_attachments {
        return false;
    }

    let (my_name, selected_peer_name, transfer_targets) = {
        let s = app.state.lock().unwrap();
        (
            s.my_username.clone(),
            s.selected_conversation.clone(),
            s.selected_transfer_targets(),
        )
    };

    if has_message {
        if app.input.ends_with('\n') {
            app.input.pop();
        }

        let content = app.input.trim().to_string();
        let now = chrono::Local::now();
        let msg = ChatMessage {
            from: my_name.clone(),
            content,
            timestamp: now.format("%H:%M").to_string(),
            timestamp_epoch: Some(now.timestamp() as u64),
            to_user: selected_peer_name.clone(),
            media: None,
            reply_to: app.replying_to.as_ref().map(|r| r.message_hash),
        };

        {
            let msg_hash = AppState::message_hash(&msg);
            let mut s = app.state.lock().unwrap();
            s.add_message(msg.clone());
            if let Some(peer_name) = &selected_peer_name {
                if !peer_name.starts_with('#') {
                    let peer_addr = s
                        .peers
                        .iter()
                        .find(|p| p.username == *peer_name)
                        .map(|p| p.addr);
                    if let Some(addr) = peer_addr {
                        s.mark_message_sent(msg_hash, addr);
                    }
                }
            }
        }

        if let Some(addr) = selected_addr {
            let _ = app.send_tx.try_send(SendRequest {
                to_addr: addr,
                message: msg,
            });
        } else {
            // Diffusion : uniquement aux pairs en ligne et joignables. On ignore
            // les pairs hors-ligne restaurés depuis l'historique (adresse nulle).
            for peer in all_peers
                .iter()
                .filter(|p| p.online && !p.addr.ip().is_unspecified())
            {
                let _ = app.send_tx.try_send(SendRequest {
                    to_addr: peer.addr,
                    message: msg.clone(),
                });
            }
        }
    }

    if has_attachments {
        // Chemin unique pour tout fichier ou dossier : streaming par morceaux.
        let targets: Vec<(String, std::net::SocketAddr)> = transfer_targets
            .iter()
            .map(|t| (t.username.clone(), t.addr))
            .collect();

        if targets.is_empty() {
            app.last_notification = Some(
                app.tr(
                    "Aucun destinataire en ligne pour l'envoi",
                    "No online recipient available",
                )
                .to_string(),
            );
            app.notification_time = std::time::Instant::now();
        } else {
            for path in app.pending_attachments.clone() {
                send_one_media(app, &path, &my_name, &selected_peer_name, &targets);
            }
        }
    }

    app.input.clear();
    app.input_cursor_char = 0;
    app.input_has_focus = true;
    app.input_scroll_lines = 0.0;
    app.pending_attachments.clear();
    app.replying_to = None;

    true
}

impl AbcomApp {
    /// Barre de saisie en bas de fenêtre. Retourne `(emoji_cliqué, gif_cliqué)`
    /// pour piloter l'ouverture des sélecteurs respectifs.
    pub(crate) fn show_input_bar(&mut self, ctx: &egui::Context) -> (bool, bool) {
        // Présence et frappe lues depuis le cache dérivé : aucune prise de
        // verrou par frame dans la barre de saisie.
        let selected_peer_online = self.sidebar_cache.selected_peer_online;

        if !selected_peer_online {
            egui::TopBottomPanel::bottom("input_panel")
                .exact_height(40.0)
                .show(ctx, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new(self.tr(
                                "🔴 Cet utilisateur est hors ligne",
                                "🔴 This user is offline",
                            ))
                            .color(egui::Color32::from_rgb(180, 40, 40))
                            .small(),
                        );
                    });
                });
            return (false, false);
        }

        let mut emoji_button_clicked = false;
        let mut gif_button_clicked = false;
        let mut picker_action: Option<AttachmentMenuAction> = None;
        let typing_list = self.sidebar_cache.typing.clone();
        let add_files_label = self.tr("Ajouter des fichiers", "Add files");
        let add_folder_label = self.tr("Ajouter un dossier", "Add folder");

        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(3.0);
                let gif_label = self.tr("GIF", "GIF");
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(66, 66, 69))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(96, 96, 100)))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            // Aperçu de réponse : extrait les données possédées
                            // avant tout appel `&mut self` (chargement de
                            // texture), pour ne pas garder `self.replying_to`
                            // emprunté pendant l'appel.
                            let reply_preview = self.replying_to.as_ref().map(|r| {
                                (
                                    r.author.clone(),
                                    r.content_snippet.clone(),
                                    r.media_thumb.clone(),
                                )
                            });
                            if let Some((author, snippet, media)) = reply_preview {
                                let reply_to_label = self.tr("Répondre à", "Replying to");
                                let texture = media
                                    .as_ref()
                                    .filter(|m| m.kind == crate::message::MediaKind::Image)
                                    .and_then(|m| self.media_texture(ctx, &m.id));
                                // Bandeau façon Discord : liseré d'accent,
                                // « Répondre à » discret, nom en gras, extrait
                                // tronqué, croix collée à droite.
                                egui::Frame::default()
                                    .fill(egui::Color32::from_rgb(52, 53, 58))
                                    .corner_radius(egui::CornerRadius::same(10))
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let (accent, _) = ui.allocate_exact_size(
                                                egui::vec2(3.0, 16.0),
                                                egui::Sense::hover(),
                                            );
                                            ui.painter().rect_filled(
                                                accent,
                                                2.0,
                                                egui::Color32::from_rgb(88, 101, 242),
                                            );
                                            ui.label(
                                                egui::RichText::new(reply_to_label)
                                                    .small()
                                                    .color(egui::Color32::from_gray(160)),
                                            );
                                            ui.label(
                                                egui::RichText::new(&author)
                                                    .small()
                                                    .color(egui::Color32::from_rgb(100, 180, 255))
                                                    .family(egui::FontFamily::Name(
                                                        super::BOLD_FAMILY.into(),
                                                    )),
                                            );
                                            if media.is_some() {
                                                super::media::render_reply_thumb(
                                                    ui,
                                                    texture.as_ref(),
                                                    20.0,
                                                );
                                            }
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if chip_remove_button(ui) {
                                                        self.replying_to = None;
                                                    }
                                                    ui.add_space(4.0);
                                                    ui.with_layout(
                                                        egui::Layout::left_to_right(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.add(
                                                                egui::Label::new(
                                                                    egui::RichText::new(&snippet)
                                                                        .small()
                                                                        .color(
                                                                            egui::Color32::from_gray(
                                                                                150,
                                                                            ),
                                                                        ),
                                                                )
                                                                .truncate(),
                                                            );
                                                        },
                                                    );
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(6.0);
                            }

                            if !self.pending_attachments.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    let mut remove_index = None;
                                    for (index, path) in self.pending_attachments.iter().enumerate()
                                    {
                                        egui::Frame::default()
                                            .fill(egui::Color32::from_rgba_unmultiplied(
                                                255, 255, 255, 24,
                                            ))
                                            .corner_radius(egui::CornerRadius::same(10))
                                            .inner_margin(egui::Margin::symmetric(8, 4))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(if path.is_dir() {
                                                        "📁"
                                                    } else {
                                                        "📄"
                                                    });
                                                    ui.label(
                                                        egui::RichText::new(attachment_label(path))
                                                            .color(egui::Color32::from_rgb(
                                                                244, 245, 247,
                                                            ))
                                                            .small(),
                                                    );
                                                    if chip_remove_button(ui) {
                                                        remove_index = Some(index);
                                                    }
                                                });
                                            });
                                    }
                                    if let Some(index) = remove_index {
                                        self.pending_attachments.remove(index);
                                    }
                                });
                                ui.add_space(6.0);
                            }

                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                                ui.set_min_height(ACTION_BUTTON_SIZE[1]);
                                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                                let plus_btn = icon_button(
                                    ui,
                                    self.tr(
                                        "Ajouter des fichiers ou dossiers",
                                        "Add files or folders",
                                    ),
                                    self.show_attachment_menu,
                                    paint_plus_icon,
                                );
                                if plus_btn.clicked() {
                                    self.show_attachment_menu = !self.show_attachment_menu;
                                }

                                let selected_addr = self.sidebar_cache.selected_peer_addr;
                                let all_peers = self.sidebar_cache.peers.clone();

                                let actions_width = 168.0;
                                let available_w = (ui.available_width() - actions_width).max(180.0);
                                let menu_open_now =
                                    emoji_shortcode_trigger(&self.input, self.input_cursor_char)
                                        .map(|(_, q)| !q.is_empty())
                                        .unwrap_or(false);

                                let (resp, mut pressed_enter, changed) =
                                    composer::custom_composer_input(
                                        ui,
                                        &mut self.input,
                                        &mut self.input_cursor_char,
                                        &mut self.input_has_focus,
                                        &mut self.input_scroll_lines,
                                        &self.emoji_map,
                                        &self.emoji_textures,
                                        &self.emoji_alias_to_char,
                                        &self.emoji_aliases,
                                        menu_open_now,
                                        self.shortcode_selected,
                                        available_w,
                                        &mut self.input_selection_anchor,
                                    );

                                ui.add_space(6.0);

                                let aa_btn = action_button(
                                    ui,
                                    egui::RichText::new("Aa")
                                        .size(11.5)
                                        .color(egui::Color32::from_rgb(244, 245, 247)),
                                    self.tr(
                                        "Mise en forme bientôt disponible",
                                        "Formatting coming soon",
                                    ),
                                    false,
                                );
                                aa_btn.clicked();

                                let gif_btn = action_button(
                                    ui,
                                    egui::RichText::new("GIF")
                                        .size(10.5)
                                        .color(egui::Color32::from_rgb(244, 245, 247)),
                                    gif_label,
                                    self.show_gif_picker,
                                );
                                if gif_btn.clicked() {
                                    if crate::config::klipy_api_key().is_some() {
                                        self.show_gif_picker = !self.show_gif_picker;
                                        self.show_emoji_picker = false;
                                        gif_button_clicked = true;
                                    } else {
                                        self.last_notification = Some(
                                            self.tr(
                                                "Clé API Klipy manquante (ABCOM_KLIPY_API_KEY)",
                                                "Klipy API key missing (ABCOM_KLIPY_API_KEY)",
                                            )
                                            .to_string(),
                                        );
                                        self.notification_time = std::time::Instant::now();
                                    }
                                }

                                let emoji_btn = if let Some((_, tex)) = self.emoji_textures.first()
                                {
                                    icon_button(
                                        ui,
                                        self.tr("Emojis", "Emoji"),
                                        self.show_emoji_picker,
                                        |painter, rect, _| {
                                            painter.image(
                                                tex.id(),
                                                rect,
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                egui::Color32::WHITE,
                                            );
                                        },
                                    )
                                } else {
                                    action_button(
                                        ui,
                                        egui::RichText::new("Em")
                                            .size(16.0)
                                            .color(egui::Color32::from_rgb(244, 245, 247)),
                                        self.tr("Emojis", "Emoji"),
                                        self.show_emoji_picker,
                                    )
                                };
                                if emoji_btn.clicked() {
                                    self.show_emoji_picker = !self.show_emoji_picker;
                                    self.show_gif_picker = false;
                                    emoji_button_clicked = true;
                                }

                                let send_btn = icon_button(
                                    ui,
                                    self.tr("Envoyer", "Send"),
                                    false,
                                    paint_send_icon,
                                );
                                if send_btn.clicked() {
                                    pressed_enter = true;
                                }

                                if self.show_attachment_menu {
                                    let popup_action = attachment_menu_popup(
                                        ctx,
                                        plus_btn.rect,
                                        add_files_label,
                                        add_folder_label,
                                    );
                                    // Estimate popup rect above the + button for outside-click detection.
                                    let popup_rect = egui::Rect::from_min_size(
                                        plus_btn.rect.left_top() - egui::vec2(0.0, 92.0),
                                        egui::vec2(216.0, 92.0),
                                    );

                                    if let Some(action) = popup_action {
                                        picker_action = Some(action);
                                        self.show_attachment_menu = false;
                                    }

                                    let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
                                        && !plus_btn.hovered()
                                        && !popup_rect.contains(ctx.input(|i| {
                                            i.pointer.interact_pos().unwrap_or_default()
                                        }));
                                    if clicked_outside {
                                        self.show_attachment_menu = false;
                                    }
                                }

                                // Popup de suggestions shortcode
                                let shortcode_limit = match emoji_shortcode_trigger(
                                    &self.input,
                                    self.input_cursor_char,
                                ) {
                                    Some((_, q)) if q.is_empty() => 5,
                                    _ => 12,
                                };
                                let shortcode_list = super::emoji_picker::shortcode_suggestions(
                                    &self.input,
                                    self.input_cursor_char,
                                    &self.emoji_alias_to_char,
                                    &self.emoji_aliases,
                                    shortcode_limit,
                                );

                                let mut clicked_shortcode: Option<String> = None;
                                if shortcode_list.is_empty() {
                                    self.shortcode_selected = 0;
                                } else if self.shortcode_selected >= shortcode_list.len() {
                                    self.shortcode_selected = shortcode_list.len() - 1;
                                }

                                // Consumir las flechas solo si el menú de shortcodes está abierto
                                if self.input_has_focus && menu_open_now {
                                    if ctx.input_mut(|i| {
                                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                                    }) && !shortcode_list.is_empty()
                                    {
                                        self.shortcode_selected = (self.shortcode_selected + 1)
                                            .min(shortcode_list.len() - 1);
                                    }
                                    if ctx.input_mut(|i| {
                                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                                    }) && !shortcode_list.is_empty()
                                    {
                                        self.shortcode_selected =
                                            self.shortcode_selected.saturating_sub(1);
                                    }
                                }

                                if self.input_has_focus && !shortcode_list.is_empty() {
                                    super::emoji_picker::show_shortcode_popup(
                                        ctx,
                                        ui,
                                        &resp,
                                        &shortcode_list,
                                        &self.emoji_map,
                                        &self.emoji_textures,
                                        self.shortcode_selected,
                                        &mut clicked_shortcode,
                                    );
                                }

                                if self.input_has_focus
                                    && !shortcode_list.is_empty()
                                    && pressed_enter
                                {
                                    if let Some((alias, _ch)) =
                                        shortcode_list.get(self.shortcode_selected)
                                    {
                                        clicked_shortcode = Some(alias.clone());
                                        pressed_enter = false;
                                    }
                                }

                                if let Some(alias) = clicked_shortcode {
                                    if let Some((start, _)) =
                                        emoji_shortcode_trigger(&self.input, self.input_cursor_char)
                                    {
                                        if let Some(ch) = self.emoji_alias_to_char.get(&alias) {
                                            let end = self.input_cursor_char;
                                            composer::replace_char_range(
                                                &mut self.input,
                                                &mut self.input_cursor_char,
                                                start,
                                                end,
                                                ch,
                                            );
                                            composer::sync_cursor(ctx, self.input_cursor_char);
                                            self.input_has_focus = true;
                                            self.show_emoji_picker = false;
                                        }
                                    }
                                }

                                // Indicateur de frappe
                                if changed
                                    && self.last_typing_broadcast.elapsed().as_millis() > 1500
                                {
                                    self.last_typing_broadcast = std::time::Instant::now();
                                    let (my_name, target_addrs) = {
                                        let s = self.state.lock().unwrap();
                                        let name = s.my_username.clone();
                                        let addrs = match &s.selected_conversation {
                                            None => s
                                                .peers
                                                .iter()
                                                .filter(|p| p.online)
                                                .map(|p| p.addr)
                                                .collect::<Vec<_>>(),
                                            Some(conv) => s
                                                .peers
                                                .iter()
                                                .find(|p| p.online && &p.username == conv)
                                                .map(|p| p.addr)
                                                .into_iter()
                                                .collect(),
                                        };
                                        (name, addrs)
                                    };
                                    for addr in target_addrs {
                                        let _ = self.send_typing_tx.try_send(TypingRequest {
                                            to_addr: addr,
                                            indicator: TypingIndicator {
                                                from: my_name.clone(),
                                            },
                                        });
                                    }
                                }

                                let pressed_enter_fallback = ui.input(|i| {
                                    i.key_pressed(egui::Key::Enter)
                                        && !i.modifiers.shift
                                        && !menu_open_now
                                });

                                if should_send_message(
                                    pressed_enter,
                                    pressed_enter_fallback,
                                    menu_open_now,
                                    &self.input,
                                ) && send_current_message(self, selected_addr, &all_peers)
                                {
                                    self.input_selection_anchor = None;
                                    resp.request_focus();
                                    self.show_emoji_picker = false;
                                }
                            });

                            if !typing_list.is_empty() {
                                ui.add_space(4.0);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Min),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} {}",
                                                typing_list.join(", "),
                                                self.tr("en train d'écrire...", "is typing...")
                                            ))
                                            .color(egui::Color32::WHITE)
                                            .small(),
                                        );
                                    },
                                );
                            }
                        });
                    });
            });

        // Defer the file/folder picker to the next frame so it runs before egui
        // rendering, avoiding an AppKit run-loop conflict on macOS.
        match picker_action {
            Some(AttachmentMenuAction::AddFiles) => {
                self.pending_picker = 1;
                ctx.request_repaint();
            }
            Some(AttachmentMenuAction::AddFolder) => {
                self.pending_picker = 2;
                ctx.request_repaint();
            }
            None => {}
        }

        (emoji_button_clicked, gif_button_clicked)
    }
}

#[cfg(test)]
#[path = "../tests/test_ui_input_bar.rs"]
mod tests;
