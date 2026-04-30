use eframe::egui;

use super::AbcomApp;
use super::composer;

/// Affiche le picker d'emojis et sa fenêtre popup
impl AbcomApp {
    pub(crate) fn show_emoji_picker_window(&mut self, ctx: &egui::Context, emoji_button_clicked: bool) {
        if !self.show_emoji_picker {
            return;
        }

        let mut picker_rect: Option<egui::Rect> = None;
        let picker_window = egui::Window::new("Emojis")
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(0.0, -60.0))
            .resizable(false)
            .collapsible(false)
            .fixed_size([310.0, 340.0]);

        if let Some(resp) = picker_window.show(ctx, |ui| {
            // Catégories
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (cat_idx, (cat_icon, _start, _end)) in crate::emoji_registry::EMOJI_CATEGORIES.iter().enumerate() {
                    let selected = self.emoji_category == cat_idx;
                    let btn = egui::Button::new(egui::RichText::new(*cat_icon).size(18.0))
                        .min_size(egui::vec2(24.0, 24.0))
                        .selected(selected)
                        .frame(selected);
                    if ui.add(btn).clicked() { self.emoji_category = cat_idx; }
                }
            });
            ui.separator();

            let (_, start, end) = crate::emoji_registry::EMOJI_CATEGORIES[self.emoji_category];
            let slice = &self.emoji_textures[start..end.min(self.emoji_textures.len())];

            egui::ScrollArea::vertical().max_height(270.0).min_scrolled_height(270.0).auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("emoji_grid").spacing([3.0, 3.0]).show(ui, |ui| {
                        for (idx, (ch, texture)) in slice.iter().enumerate() {
                            let (cell_rect, cell_resp) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::click());
                            if cell_resp.hovered() {
                                ui.painter().rect_filled(cell_rect, 6.0, ui.visuals().widgets.hovered.bg_fill);
                            }
                            ui.painter().image(texture.id(), cell_rect.shrink(1.0),
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE);
                            if cell_resp.on_hover_text(ch.as_str()).clicked() {
                                composer::insert_emoji_at_cursor(&mut self.input, &mut self.input_cursor_char, ch);
                                self.show_emoji_picker = false;
                            }
                            if (idx + 1) % 8 == 0 { ui.end_row(); }
                        }
                    });
                });
        }) {
            picker_rect = Some(resp.response.rect);
        }

        if !emoji_button_clicked && ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if let Some(rect) = picker_rect {
                    if !rect.contains(pos) {
                        self.show_emoji_picker = false;
                    }
                }
            }
        }
    }
}

// ─── Utilitaires de shortcode réutilisés depuis input_bar ───────────────────

/// Détecte si le curseur est dans un shortcode `:xxx`
pub(crate) fn emoji_shortcode_trigger(input: &str, cursor_char: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let end = cursor_char.min(chars.len());
    let slice: String = chars[..end].iter().collect();
    if let Some(colon_pos) = slice.rfind(':') {
        let after = &slice[colon_pos + 1..];
        if !after.contains(' ') && !after.contains(':') {
            let start_char = slice[..colon_pos].chars().count();
            return Some((start_char, after.to_string()));
        }
    }
    None
}

/// Suggestions de shortcodes à partir d'un préfixe
pub(crate) fn shortcode_suggestions(
    input: &str,
    cursor_char: usize,
    alias_to_char: &std::collections::HashMap<String, String>,
    aliases: &[String],
    limit: usize,
) -> Vec<(String, String)> {
    let Some((_start, query)) = emoji_shortcode_trigger(input, cursor_char) else {
        return Vec::new();
    };
    aliases.iter()
        .filter(|a| a.starts_with(&query))
        .take(limit)
        .filter_map(|a| alias_to_char.get(a).map(|ch| (a.clone(), ch.clone())))
        .collect()
}

/// Affiche la popup de suggestions de shortcodes
pub(crate) fn show_shortcode_popup(
    ctx: &egui::Context,
    _ui: &mut egui::Ui,
    resp: &egui::Response,
    shortcode_list: &[(String, String)],
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
    shortcode_selected: usize,
    clicked_shortcode: &mut Option<String>,
) {
    let row_h = 28.0;
    let desired_h = (shortcode_list.len() as f32 * row_h + 8.0).min(220.0);
    let screen = ctx.screen_rect();
    let gap = 14.0;
    let popup_bottom = resp.rect.top() - gap;
    let available_above = (popup_bottom - (screen.top() + 4.0)).max(0.0);
    let popup_h = desired_h.min(available_above);
    if popup_h <= 8.0 { return; }
    let popup_w = resp.rect.width().max(260.0);
    let popup_y = popup_bottom - popup_h;
    egui::Area::new("emoji_shortcode_popup".into())
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(resp.rect.left(), popup_y))
        .show(ctx, |ui| {
            ui.set_min_size(egui::vec2(popup_w, popup_h));
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(popup_w);
                egui::ScrollArea::vertical().auto_shrink([false, false])
                    .max_height((popup_h - 8.0).max(row_h))
                    .show_rows(ui, row_h, shortcode_list.len(), |ui, row_range| {
                        for idx in row_range {
                            let (alias, ch) = &shortcode_list[idx];
                            let row_w = ui.available_width().max(popup_w - 16.0);
                            let (row_rect, row_resp) = ui.allocate_exact_size(egui::vec2(row_w, row_h), egui::Sense::click());
                            let hovered = row_resp.hovered();
                            let selected = idx == shortcode_selected;
                            if hovered || selected {
                                let fill = if hovered { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.active.bg_fill.gamma_multiply(0.6) };
                                ui.painter().rect_filled(row_rect, 4.0, fill);
                            }
                            let mut x = row_rect.left() + 8.0;
                            let y = row_rect.center().y;
                            if let Some(&tex_idx) = emoji_map.get(ch) {
                                if let Some((_, tex)) = emoji_textures.get(tex_idx) {
                                    let img_rect = egui::Rect::from_center_size(egui::pos2(x + 9.0, y), egui::vec2(18.0, 18.0));
                                    ui.painter().image(tex.id(), img_rect,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE);
                                }
                            }
                            x += 26.0;
                            let text_color = if selected { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                            ui.painter().text(egui::pos2(x, y), egui::Align2::LEFT_CENTER, format!(":{}", alias),
                                egui::TextStyle::Body.resolve(ui.style()), text_color);
                            if row_resp.clicked() { *clicked_shortcode = Some(alias.clone()); }
                        }
                    });
            });
        });
}

/// Rendu inline d'un texte avec emojis PNG
pub(crate) fn render_inline(
    ui: &mut egui::Ui,
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    textures: &[(String, egui::TextureHandle)],
    emoji_size: f32,
) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut acc = String::new();
    let size = egui::vec2(emoji_size, emoji_size);
    while i < chars.len() {
        let mut matched = false;
        for len in [2usize, 1] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if let Some(&idx) = emoji_map.get(&s) {
                    if !acc.is_empty() { ui.label(&acc); acc.clear(); }
                    if let Some((_, tex)) = textures.get(idx) {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(size));
                    }
                    i += len;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            let ch = chars[i];
            if ch != '\u{fe0f}' && ch != '\u{200d}' { acc.push(ch); }
            i += 1;
        }
    }
    if !acc.is_empty() { ui.label(&acc); }
}

/// Construit l'index shortcode → emoji
pub(crate) fn build_emoji_shortcode_index(
    available: &[String],
) -> (std::collections::HashMap<String, String>, Vec<String>) {
    let available_set: std::collections::HashSet<String> = available.iter().cloned().collect();
    let Ok(raw): Result<std::collections::HashMap<String, serde_json::Value>, _> =
        serde_json::from_str(include_str!("../../assets/github.raw.json"))
    else {
        return (std::collections::HashMap::new(), Vec::new());
    };

    let mut alias_to_char = std::collections::HashMap::new();
    for (key, value) in raw {
        let Some(emoji) = github_key_to_emoji(&key) else { continue; };
        if !available_set.contains(&emoji) { continue; }
        if let Some(aliases) = value.as_array() {
            for alias in aliases {
                if let Some(a) = alias.as_str() {
                    alias_to_char.insert(a.to_string(), emoji.clone());
                }
            }
        } else if let Some(alias) = value.as_str() {
            alias_to_char.insert(alias.to_string(), emoji);
        }
    }

    let mut aliases: Vec<String> = alias_to_char.keys().cloned().collect();
    aliases.sort();
    (alias_to_char, aliases)
}

fn github_key_to_emoji(key: &str) -> Option<String> {
    let mut out = String::new();
    for part in key.split('-') {
        let cp = u32::from_str_radix(part, 16).ok()?;
        out.push(char::from_u32(cp)?);
    }
    Some(out)
}
