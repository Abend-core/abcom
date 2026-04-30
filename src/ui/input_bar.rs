use eframe::egui;

use crate::app::AppState;
use crate::message::{ChatMessage, SendRequest, TypingIndicator, TypingRequest};

use super::composer;
use super::emoji_picker::emoji_shortcode_trigger;
use super::{AbcomApp, AppView};

fn should_send_message(
    pressed_enter: bool,
    pressed_enter_fallback: bool,
    shortcode_menu_open: bool,
    input: &str,
) -> bool {
    (pressed_enter || (pressed_enter_fallback && !shortcode_menu_open)) && !input.trim().is_empty()
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
                Some(u) => s.is_peer_online(u),
            }
        };

        if !selected_peer_online {
            egui::TopBottomPanel::bottom("input_panel")
                .exact_height(40.0)
                .show(ctx, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("🔴 Cet utilisateur est hors ligne")
                                .color(egui::Color32::from_rgb(180, 40, 40))
                                .small(),
                        );
                    });
                });
            return false;
        }

        let mut emoji_button_clicked = false;

        egui::TopBottomPanel::bottom("input_panel")
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);

                    // Bouton emoji
                    let emoji_btn = ui
                        .allocate_ui_with_layout(
                            egui::vec2(24.0, 24.0),
                            egui::Layout::centered_and_justified(egui::Direction::TopDown),
                            |ui| {
                                if !self.emoji_textures.is_empty() {
                                    let (_ch, tex) = &self.emoji_textures[0];
                                    let (btn_rect, btn_resp) = ui.allocate_exact_size(
                                        egui::vec2(24.0, 24.0),
                                        egui::Sense::click(),
                                    );
                                    if self.show_emoji_picker {
                                        ui.painter().rect_filled(
                                            btn_rect,
                                            6.0,
                                            ui.visuals().selection.bg_fill,
                                        );
                                    }
                                    ui.painter().image(
                                        tex.id(),
                                        btn_rect.shrink(3.0),
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        egui::Color32::WHITE,
                                    );
                                    btn_resp
                                } else {
                                    ui.button("😊")
                                }
                            },
                        )
                        .inner;
                    if emoji_btn.clicked() {
                        self.show_emoji_picker = !self.show_emoji_picker;
                        emoji_button_clicked = true;
                    }

                    ui.add_space(4.0);

                    let (selected_addr, all_peers) = {
                        let s = self.state.lock().unwrap();
                        (s.selected_peer_addr(), s.peers.clone())
                    };

                    let available_w = ui.available_width() - 8.0;
                    let menu_open_now =
                        emoji_shortcode_trigger(&self.input, self.input_cursor_char)
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
                        available_w - 12.0,
                    );

                    // Popup de suggestions shortcode
                    let shortcode_limit =
                        match emoji_shortcode_trigger(&self.input, self.input_cursor_char) {
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

                    ui.add_space(4.0);
                    let pressed_enter_fallback = ui.input(|i| {
                        i.key_pressed(egui::Key::Enter) && !i.modifiers.shift && !menu_open_now
                    });

                    if should_send_message(
                        pressed_enter,
                        pressed_enter_fallback,
                        menu_open_now,
                        &self.input,
                    ) {
                        if self.input.ends_with('\n') {
                            self.input.pop();
                        }
                        let content = self.input.trim().to_string();
                        let now = chrono::Local::now().format("%H:%M").to_string();
                        let (my_name, selected_peer_name) = {
                            let s = self.state.lock().unwrap();
                            (s.my_username.clone(), s.selected_conversation.clone())
                        };

                        let msg = ChatMessage {
                            from: my_name,
                            content,
                            timestamp: now,
                            to_user: selected_peer_name.clone(),
                        };

                        {
                            let msg_hash = AppState::message_hash(&msg);
                            let mut s = self.state.lock().unwrap();
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

                        self.input.clear();
                        self.input_cursor_char = 0;
                        self.input_has_focus = true;
                        self.input_scroll_lines = 0.0;

                        if let Some(addr) = selected_addr {
                            let _ = self.send_tx.try_send(SendRequest {
                                to_addr: addr,
                                message: msg,
                            });
                        } else {
                            for peer in &all_peers {
                                let _ = self.send_tx.try_send(SendRequest {
                                    to_addr: peer.addr,
                                    message: msg.clone(),
                                });
                            }
                        }

                        resp.request_focus();
                        self.show_emoji_picker = false;
                    }
                });
            });

        emoji_button_clicked
    }
}

#[cfg(test)]
mod tests {
    use super::should_send_message;

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
}
