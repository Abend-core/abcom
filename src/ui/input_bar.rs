use eframe::egui;
use std::path::{Path, PathBuf};

use crate::app::AppState;
use crate::message::{ChatMessage, SendRequest, TypingIndicator, TypingRequest};
use crate::transfer::TransferRequest;

use super::composer;
use super::emoji_picker::emoji_shortcode_trigger;
use super::{AbcomApp, AppView};

const ACTION_BUTTON_SIZE: [f32; 2] = [34.0, 34.0];

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

fn paint_plus_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    let arm = rect.width().min(rect.height()) * 0.34;
    let stroke = egui::Stroke::new(2.0, color);
    painter.line_segment(
        [egui::pos2(center.x - arm, center.y), egui::pos2(center.x + arm, center.y)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(center.x, center.y - arm), egui::pos2(center.x, center.y + arm)],
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
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(102, 102, 108)))
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
        let now = chrono::Local::now().format("%H:%M").to_string();
        let msg = ChatMessage {
            from: my_name.clone(),
            content,
            timestamp: now,
            to_user: selected_peer_name.clone(),
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
            for peer in all_peers {
                let _ = app.send_tx.try_send(SendRequest {
                    to_addr: peer.addr,
                    message: msg.clone(),
                });
            }
        }
    }

    if has_attachments {
        let paths = app.pending_attachments.clone();
        for target in &transfer_targets {
            let _ = app.send_transfer_tx.try_send(TransferRequest {
                from: my_name.clone(),
                recipient: target.username.clone(),
                to_addr: target.addr,
                paths: paths.clone(),
            });
        }
        if transfer_targets.is_empty() {
            app.last_notification = Some(
                app.tr(
                    "Aucun destinataire en ligne pour le transfert",
                    "No online recipient available for transfer",
                )
                .to_string(),
            );
            app.notification_time = std::time::Instant::now();
        }
    }

    app.input.clear();
    app.input_cursor_char = 0;
    app.input_has_focus = true;
    app.input_scroll_lines = 0.0;
    app.pending_attachments.clear();

    true
}

impl AbcomApp {
    /// Barre de saisie en bas de fenêtre. Retourne true si le bouton emoji a été cliqué.
    pub(crate) fn show_input_bar(&mut self, ctx: &egui::Context) -> bool {
        // Ne pas afficher la barre de saisie en Networks view
        if self.active_view == AppView::Networks {
            return false;
        }

        let selected_peer_online = {
            let s = self.state.lock().unwrap();
            match &s.selected_conversation {
                None => true,
                Some(conv) if conv.starts_with('#') => true,
                Some(u) => s.is_peer_online(u),
            }
        };

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
            return false;
        }

        let mut emoji_button_clicked = false;
        let mut picker_action: Option<AttachmentMenuAction> = None;
        let add_files_label = self.tr("Ajouter des fichiers", "Add files");
        let add_folder_label = self.tr("Ajouter un dossier", "Add folder");

        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(3.0);
                let gif_soon_label = self.tr("GIF bientôt disponible", "GIF support coming soon");
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(66, 66, 69))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(96, 96, 100)))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            if !self.pending_attachments.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    let mut remove_index = None;
                                    for (index, path) in self.pending_attachments.iter().enumerate() {
                                        egui::Frame::default()
                                            .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24))
                                            .corner_radius(egui::CornerRadius::same(10))
                                            .inner_margin(egui::Margin::symmetric(8, 4))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(if path.is_dir() { "📁" } else { "📄" });
                                                    ui.label(
                                                        egui::RichText::new(attachment_label(path))
                                                            .color(egui::Color32::from_rgb(244, 245, 247))
                                                            .small(),
                                                    );
                                                    if ui.small_button("✕").clicked() {
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

                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Min),
                                |ui| {
                                ui.set_min_height(ACTION_BUTTON_SIZE[1]);
                                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                                let plus_btn = icon_button(
                                    ui,
                                    self.tr("Ajouter des fichiers ou dossiers", "Add files or folders"),
                                    self.show_attachment_menu,
                                    paint_plus_icon,
                                );
                                if plus_btn.clicked() {
                                    self.show_attachment_menu = !self.show_attachment_menu;
                                }

                                let (selected_addr, all_peers) = {
                                    let s = self.state.lock().unwrap();
                                    (s.selected_peer_addr(), s.peers.clone())
                                };

                                let actions_width = 168.0;
                                let available_w = (ui.available_width() - actions_width).max(180.0);
                                let menu_open_now = emoji_shortcode_trigger(&self.input, self.input_cursor_char)
                                    .map(|(_, q)| !q.is_empty())
                                    .unwrap_or(false);

                                let (resp, mut pressed_enter, changed) = composer::custom_composer_input(
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
                                if aa_btn.clicked() {
                                    self.last_notification = Some(
                                        self.tr(
                                            "Mise en forme bientôt disponible",
                                            "Formatting coming soon",
                                        )
                                        .to_string(),
                                    );
                                    self.notification_time = std::time::Instant::now();
                                }

                                let image_btn = action_button(
                                    ui,
                                    egui::RichText::new("GIF")
                                        .size(10.5)
                                        .color(egui::Color32::from_rgb(244, 245, 247)),
                                    gif_soon_label,
                                    false,
                                );
                                if image_btn.clicked() {
                                    self.last_notification = Some(gif_soon_label.to_string());
                                    self.notification_time = std::time::Instant::now();
                                }

                                let emoji_btn = if let Some((_, tex)) = self.emoji_textures.first() {
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
                                        egui::RichText::new("☺")
                                            .size(16.0)
                                            .color(egui::Color32::from_rgb(244, 245, 247)),
                                        self.tr("Emojis", "Emoji"),
                                        self.show_emoji_picker,
                                    )
                                };
                                if emoji_btn.clicked() {
                                    self.show_emoji_picker = !self.show_emoji_picker;
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
                                        && !popup_rect.contains(
                                            ctx.input(|i| i.pointer.interact_pos().unwrap_or_default()),
                                        );
                                    if clicked_outside {
                                        self.show_attachment_menu = false;
                                    }
                                }

                            // Popup de suggestions shortcode
                            let shortcode_limit = match emoji_shortcode_trigger(&self.input, self.input_cursor_char) {
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
                        }) {
                            if !shortcode_list.is_empty() {
                                self.shortcode_selected =
                                    (self.shortcode_selected + 1).min(shortcode_list.len() - 1);
                            }
                        }
                        if ctx
                            .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
                        {
                            if !shortcode_list.is_empty() {
                                self.shortcode_selected = self.shortcode_selected.saturating_sub(1);
                            }
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

                            if self.input_has_focus && !shortcode_list.is_empty() && pressed_enter {
                        if let Some((alias, _ch)) = shortcode_list.get(self.shortcode_selected) {
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
                            if changed && self.last_typing_broadcast.elapsed().as_millis() > 1500 {
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
                                i.key_pressed(egui::Key::Enter) && !i.modifiers.shift && !menu_open_now
                            });

                                if should_send_message(
                                    pressed_enter,
                                    pressed_enter_fallback,
                                    menu_open_now,
                                    &self.input,
                                ) {
                                    if send_current_message(self, selected_addr, &all_peers) {
                                        resp.request_focus();
                                        self.show_emoji_picker = false;
                                    }
                                }
                            },
                            );
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

        emoji_button_clicked
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{attachment_label, push_unique_paths, should_send_message};

    #[test]
    fn enter_from_composer_sends_when_shortcode_menu_is_closed() {
        assert!(should_send_message(true, false, false, "hello"));
    }

    #[test]
    fn enter_fallback_does_not_send_when_shortcode_menu_is_open() {
        assert!(!should_send_message(false, true, true, ":jo"));
    }

    #[test]
    fn enter_fallback_sends_when_shortcode_menu_is_closed() {
        assert!(should_send_message(false, true, false, "hello"));
    }

    #[test]
    fn empty_message_never_sends() {
        assert!(!should_send_message(true, true, false, "   "));
    }

    #[test]
    fn push_unique_paths_ignores_duplicates() {
        let mut paths = vec![PathBuf::from("/tmp/alpha.txt")];

        push_unique_paths(
            &mut paths,
            [
                PathBuf::from("/tmp/alpha.txt"),
                PathBuf::from("/tmp/beta.txt"),
                PathBuf::from("/tmp/beta.txt"),
            ],
        );

        assert_eq!(
            paths,
            vec![PathBuf::from("/tmp/alpha.txt"), PathBuf::from("/tmp/beta.txt")]
        );
    }

    #[test]
    fn attachment_label_prefers_file_name() {
        assert_eq!(
            attachment_label(PathBuf::from("/tmp/subdir/report.pdf").as_path()),
            "report.pdf"
        );
    }

    #[test]
    fn attachment_label_falls_back_to_full_path_when_needed() {
        let path = PathBuf::from("/");
        assert_eq!(attachment_label(path.as_path()), "/");
    }
}
