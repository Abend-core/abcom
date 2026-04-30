pub mod cursor;
pub mod render;
pub mod shortcode;
pub mod text_ops;

pub use cursor::{composer_caret_positions, cursor_from_point, move_cursor_vertical};
pub use render::{render_inline, measure_text_width};
pub use shortcode::{emoji_shortcode_trigger, shortcode_suggestions};
pub use text_ops::{
    char_to_byte_idx, insert_emoji_at_cursor, insert_text_at_cursor,
    remove_next_char, remove_prev_char, replace_char_range,
};

use eframe::egui;

/// Widget de composition de message multi-ligne avec rendu inline emoji.
/// Retourne (Response, pressed_enter: bool, changed: bool)
pub fn custom_composer_input(
    ui: &mut egui::Ui,
    text: &mut String,
    cursor_char: &mut usize,
    has_focus: &mut bool,
    scroll_lines: &mut f32,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
    alias_to_char: &std::collections::HashMap<String, String>,
    aliases: &[String],
    shortcode_menu_open: bool,
    available_width: f32,
) -> (egui::Response, bool, bool) {
    let line_height = 22.0;
    let lines = text.chars().filter(|&c| c == '\n').count() as f32 + 1.0;
    let vis_lines = lines.clamp(1.0, 10.0);
    let height = vis_lines * line_height + 10.0;

    let (rect, resp) = ui.allocate_exact_size(egui::vec2(available_width, height), egui::Sense::click_and_drag());

    if resp.clicked() { *has_focus = true; }
    if ui.input(|i| i.pointer.primary_pressed()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if !rect.contains(pos) { *has_focus = false; }
        }
    }

    let visuals = ui.style().interact(&resp);
    let bg = if *has_focus { ui.visuals().extreme_bg_color } else { ui.visuals().faint_bg_color };
    ui.painter().rect_filled(rect, 8.0, bg);
    ui.painter().rect_stroke(rect, 8.0, if *has_focus { ui.visuals().selection.stroke } else { visuals.bg_stroke }, egui::StrokeKind::Outside);

    let mut pressed_enter = false;
    let mut changed = false;

    // Handle text input
    if *has_focus {
        let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
        for event in &events {
            match event {
                egui::Event::Text(t) => {
                    if !t.is_empty() && !t.starts_with('\r') {
                        insert_text_at_cursor(text, cursor_char, t);
                        changed = true;
                    }
                }
                egui::Event::Key { key, pressed: true, modifiers, .. } => {
                    if *key == egui::Key::Enter && !modifiers.shift {
                        if shortcode_menu_open {
                            // defer to shortcode menu
                        } else {
                            pressed_enter = true;
                        }
                    } else if *key == egui::Key::Enter && modifiers.shift {
                        insert_text_at_cursor(text, cursor_char, "\n");
                        changed = true;
                    } else if *key == egui::Key::Backspace {
                        if modifiers.ctrl {
                            // delete word
                            let new_cursor = prev_word_start(text, *cursor_char);
                            let bytes_start = char_to_byte_idx(text, new_cursor);
                            let bytes_end = char_to_byte_idx(text, *cursor_char);
                            text.replace_range(bytes_start..bytes_end, "");
                            *cursor_char = new_cursor;
                        } else {
                            remove_prev_char(text, cursor_char);
                        }
                        changed = true;
                    } else if *key == egui::Key::Delete {
                        remove_next_char(text, cursor_char);
                        changed = true;
                    } else if *key == egui::Key::ArrowLeft {
                        if *cursor_char > 0 {
                            if modifiers.ctrl { *cursor_char = prev_word_start(text, *cursor_char); }
                            else { *cursor_char -= 1; }
                        }
                    } else if *key == egui::Key::ArrowRight {
                        let total = text.chars().count();
                        if *cursor_char < total {
                            if modifiers.ctrl { *cursor_char = next_word_end(text, *cursor_char); }
                            else { *cursor_char += 1; }
                        }
                    } else if *key == egui::Key::Home {
                        let line_start = prev_line_start(text, *cursor_char);
                        *cursor_char = line_start;
                    } else if *key == egui::Key::End {
                        *cursor_char = next_line_end(text, *cursor_char);
                    } else if *key == egui::Key::ArrowUp && !shortcode_menu_open {
                        *cursor_char = move_cursor_vertical(text, *cursor_char, -1, rect.left(), rect.top(), line_height, emoji_map, emoji_textures, ui.ctx());
                    } else if *key == egui::Key::ArrowDown && !shortcode_menu_open {
                        *cursor_char = move_cursor_vertical(text, *cursor_char, 1, rect.left(), rect.top(), line_height, emoji_map, emoji_textures, ui.ctx());
                    }
                }
                egui::Event::Paste(s) => {
                    insert_text_at_cursor(text, cursor_char, s);
                    changed = true;
                }
                egui::Event::Copy | egui::Event::Cut => {}
                _ => {}
            }
        }
    }

    // Clamp cursor
    let total_chars = text.chars().count();
    *cursor_char = (*cursor_char).min(total_chars);

    // Render text + emoji + cursor
    let inner_rect = rect.shrink2(egui::vec2(8.0, 5.0));
    let mut child = ui.child_ui(inner_rect, egui::Layout::left_to_right(egui::Align::Min), None);
    child.set_clip_rect(inner_rect);

    // Render content (plain text with emoji)
    let (lines_vec, _) = composer_caret_positions(text, emoji_map, emoji_textures, &child, line_height);

    let y_start = inner_rect.top() - (*scroll_lines * line_height);
    let mut x = inner_rect.left();
    let mut y = y_start;
    let default_font = egui::TextStyle::Body.resolve(child.style());
    let mut cursor_pixel: Option<egui::Pos2> = None;

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        // emoji detection (2-char then 1-char)
        let mut drawn = false;
        for len in [2usize, 1usize] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if let Some(&tex_idx) = emoji_map.get(&s) {
                    if i == *cursor_char && *has_focus {
                        cursor_pixel = Some(egui::pos2(x, y));
                    }
                    if let Some((_, tex)) = emoji_textures.get(tex_idx) {
                        let img_rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(line_height, line_height));
                        if inner_rect.intersects(img_rect) {
                            ui.painter().image(tex.id(), img_rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE);
                        }
                    }
                    x += line_height;
                    i += len;
                    drawn = true;
                    break;
                }
            }
        }
        if !drawn {
            if ch == '\u{fe0f}' || ch == '\u{200d}' { i += 1; continue; }
            if ch == '\n' {
                if i == *cursor_char && *has_focus {
                    cursor_pixel = Some(egui::pos2(x, y));
                }
                x = inner_rect.left();
                y += line_height;
                i += 1;
                continue;
            }
            if i == *cursor_char && *has_focus {
                cursor_pixel = Some(egui::pos2(x, y));
            }
            let glyph: String = std::iter::once(ch).collect();
            let glyph_w = measure_text_width(&glyph, ui.ctx(), &default_font);
            let text_pos = egui::pos2(x, y);
            if inner_rect.y_range().contains(y) {
                ui.painter().text(text_pos, egui::Align2::LEFT_TOP, &glyph, default_font.clone(), ui.visuals().text_color());
            }
            x += glyph_w;
            i += 1;
        }
    }
    if *cursor_char == chars.len() && *has_focus {
        cursor_pixel = Some(egui::pos2(x, y));
    }

    // Draw caret
    if *has_focus {
        if let Some(pos) = cursor_pixel {
            let blink = (ui.input(|i| i.time) * 2.0).floor() as i32 % 2 == 0;
            if blink {
                ui.painter().line_segment(
                    [pos, pos + egui::vec2(0.0, line_height - 2.0)],
                    egui::Stroke::new(1.5, ui.visuals().text_color()),
                );
            }
        }
    }

    let _ = lines_vec;
    (resp, pressed_enter, changed)
}

fn prev_word_start(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = cursor;
    while i > 0 && chars[i - 1] == ' ' { i -= 1; }
    while i > 0 && chars[i - 1] != ' ' { i -= 1; }
    i
}

fn next_word_end(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = cursor;
    while i < chars.len() && chars[i] != ' ' { i += 1; }
    while i < chars.len() && chars[i] == ' ' { i += 1; }
    i
}

fn prev_line_start(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = cursor;
    if i == 0 { return 0; }
    while i > 0 && chars[i - 1] != '\n' { i -= 1; }
    i
}

fn next_line_end(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = cursor;
    while i < chars.len() && chars[i] != '\n' { i += 1; }
    i
}
