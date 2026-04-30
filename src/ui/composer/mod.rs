#[allow(dead_code)]
pub mod cursor;
pub mod render;
pub mod shortcode;
pub mod text_ops;

pub use text_ops::{insert_emoji_at_cursor, replace_char_range};

use eframe::egui;

use self::text_ops::{insert_text_at_cursor, remove_next_char, remove_prev_char};

pub fn sync_cursor(_ctx: &egui::Context, _char_pos: usize) {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnterKeyAction {
    InsertNewline,
    AcceptShortcode,
    Submit,
}

fn enter_key_action(shortcode_menu_open: bool, shift: bool) -> EnterKeyAction {
    if shift {
        EnterKeyAction::InsertNewline
    } else if shortcode_menu_open {
        EnterKeyAction::AcceptShortcode
    } else {
        EnterKeyAction::Submit
    }
}

fn accept_selected_shortcode(
    input: &mut String,
    cursor_char: &mut usize,
    emoji_alias_to_char: &std::collections::HashMap<String, String>,
    emoji_aliases: &[String],
    shortcode_selected: usize,
) -> bool {
    let Some((start, _query)) =
        crate::ui::emoji_picker::emoji_shortcode_trigger(input, *cursor_char)
    else {
        return false;
    };
    let suggestions = crate::ui::emoji_picker::shortcode_suggestions(
        input,
        *cursor_char,
        emoji_alias_to_char,
        emoji_aliases,
        shortcode_selected.saturating_add(1),
    );
    let Some((_alias, ch)) =
        suggestions.get(shortcode_selected.min(suggestions.len().saturating_sub(1)))
    else {
        return false;
    };

    replace_char_range(input, cursor_char, start, *cursor_char, ch);
    true
}

fn measure_text_width(ui: &egui::Ui, text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font_id, ui.visuals().text_color())
        .size()
        .x
}

fn composer_caret_positions(
    ui: &egui::Ui,
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_size: f32,
    max_width: f32,
) -> Vec<egui::Pos2> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let line_height = 22.0;
    let mut x = 0.0;
    let mut y = 0.0;
    let mut points = Vec::with_capacity(chars.len() + 1);
    points.push(egui::pos2(0.0, 0.0));

    while i < chars.len() {
        if chars[i] == '\n' {
            x = 0.0;
            y += line_height;
            i += 1;
            points.push(egui::pos2(x, y));
            continue;
        }

        let mut matched = false;
        for len in [2usize, 1usize] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if emoji_map.contains_key(&s) {
                    let advance = emoji_size + 2.0;
                    if x + advance > max_width && x > 0.0 {
                        x = 0.0;
                        y += line_height;
                    }
                    x += advance;
                    for _ in 0..len {
                        points.push(egui::pos2(x, y));
                    }
                    i += len;
                    matched = true;
                    break;
                }
            }
        }

        if !matched {
            let ch = chars[i].to_string();
            let advance = measure_text_width(ui, &ch);
            if x + advance > max_width && x > 0.0 {
                x = 0.0;
                y += line_height;
            }
            x += advance;
            i += 1;
            points.push(egui::pos2(x, y));
        }
    }

    points
}

fn visual_line_count(caret_points: &[egui::Pos2], line_height: f32) -> usize {
    caret_points
        .last()
        .map(|p| (p.y / line_height).floor() as usize + 1)
        .unwrap_or(1)
        .max(1)
}

fn cursor_from_point(points: &[egui::Pos2], target: egui::Pos2) -> usize {
    let mut best_idx = 0;
    let mut best_dist = f32::MAX;

    for (idx, p) in points.iter().enumerate() {
        let dx = p.x - target.x;
        let dy = p.y - target.y;
        let dist = dx * dx + dy * dy;
        if dist < best_dist {
            best_dist = dist;
            best_idx = idx;
        }
    }

    best_idx
}

fn move_cursor_vertical(
    points: &[egui::Pos2],
    cursor_char: &mut usize,
    delta_lines: i32,
    line_height: f32,
) {
    if points.is_empty() {
        return;
    }

    let current = points
        .get(*cursor_char)
        .copied()
        .unwrap_or_else(|| *points.last().unwrap_or(&egui::pos2(0.0, 0.0)));
    let current_line = (current.y / line_height).round() as i32;
    let target_line = current_line + delta_lines;
    if target_line < 0 {
        return;
    }

    let mut best_idx = None;
    let mut best_dist = f32::MAX;
    for (idx, p) in points.iter().enumerate() {
        let line = (p.y / line_height).round() as i32;
        if line == target_line {
            let dist = (p.x - current.x).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(idx);
            }
        }
    }

    if let Some(idx) = best_idx {
        *cursor_char = idx;
    }
}

pub fn custom_composer_input(
    ui: &mut egui::Ui,
    input: &mut String,
    cursor_char: &mut usize,
    input_has_focus: &mut bool,
    scroll_lines: &mut f32,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
    emoji_alias_to_char: &std::collections::HashMap<String, String>,
    emoji_aliases: &[String],
    shortcode_menu_open: bool,
    shortcode_selected: usize,
    width: f32,
) -> (egui::Response, bool, bool) {
    let line_height = 22.0;
    let base_content_width = (width.max(120.0) - 12.0).max(20.0);
    let initial_caret_points =
        composer_caret_positions(ui, input, emoji_map, 18.0, base_content_width);
    let mut line_count = visual_line_count(&initial_caret_points, line_height);
    let needs_scrollbar = line_count > 10;
    let content_width = if needs_scrollbar {
        (width.max(120.0) - 20.0).max(20.0)
    } else {
        base_content_width
    };
    if needs_scrollbar {
        let scrollbar_caret_points =
            composer_caret_positions(ui, input, emoji_map, 18.0, content_width);
        line_count = visual_line_count(&scrollbar_caret_points, line_height);
    }
    let visual_lines = line_count.clamp(1, 10) as f32;
    let desired_size = egui::vec2(width.max(120.0), 16.0 + visual_lines * line_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let content_rect = if needs_scrollbar {
        egui::Rect::from_min_max(
            rect.min + egui::vec2(6.0, 6.0),
            rect.max - egui::vec2(14.0, 6.0),
        )
    } else {
        rect.shrink2(egui::vec2(6.0, 6.0))
    };
    let caret_points =
        composer_caret_positions(ui, input, emoji_map, 18.0, content_rect.width().max(20.0));
    let max_scroll = (line_count as f32 - visual_lines).max(0.0);
    *scroll_lines = scroll_lines.clamp(0.0, max_scroll);

    if response.clicked() {
        *input_has_focus = true;
        response.request_focus();
        if let Some(pos) = response.interact_pointer_pos() {
            let local = egui::pos2(
                (pos.x - content_rect.left()).max(0.0),
                (pos.y - content_rect.top()).max(0.0),
            );
            *cursor_char = cursor_from_point(&caret_points, local);
        } else {
            *cursor_char = input.chars().count();
        }
    }

    let has_focus = *input_has_focus || response.has_focus();
    let mut changed = false;
    let mut submit = false;
    let total_chars = input.chars().count();
    if *cursor_char > total_chars {
        *cursor_char = total_chars;
    }

    if let Some(caret) = caret_points.get(*cursor_char) {
        let caret_line = (caret.y / line_height).floor();
        if caret_line < *scroll_lines {
            *scroll_lines = caret_line;
        }
        if caret_line >= *scroll_lines + visual_lines {
            *scroll_lines = caret_line - visual_lines + 1.0;
        }
        *scroll_lines = scroll_lines.clamp(0.0, max_scroll);
    }

    if has_focus {
        let caret = caret_points
            .get(*cursor_char)
            .copied()
            .unwrap_or_else(|| *caret_points.last().unwrap_or(&egui::pos2(0.0, 0.0)));
        let cursor_x = content_rect.left() + caret.x + 1.0;
        let cursor_top = content_rect.top() + caret.y + 2.0 - (*scroll_lines * line_height);
        let cursor_bottom = (cursor_top + 18.0).min(content_rect.bottom() - 2.0);
        ui.ctx().output_mut(|o| {
            o.mutable_text_under_cursor = true;
            o.ime = Some(egui::output::IMEOutput {
                rect,
                cursor_rect: egui::Rect::from_min_max(
                    egui::pos2(cursor_x, cursor_top.max(content_rect.top())),
                    egui::pos2(
                        cursor_x + 1.0,
                        cursor_bottom.max(cursor_top.max(content_rect.top())),
                    ),
                ),
            });
        });

        if response.hovered() {
            let wheel_y = ui.input(|i| i.raw_scroll_delta.y + i.smooth_scroll_delta.y);
            if wheel_y.abs() > 0.0 && max_scroll > 0.0 {
                *scroll_lines = (*scroll_lines - wheel_y / 32.0).clamp(0.0, max_scroll);
            }
        }

        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(t) => {
                    if !t.contains('\n') && !t.contains('\r') {
                        insert_text_at_cursor(input, cursor_char, &t);
                        changed = true;
                    }
                }
                egui::Event::Ime(egui::ImeEvent::Commit(t)) => {
                    if !t.contains('\n') && !t.contains('\r') && !t.is_empty() {
                        insert_text_at_cursor(input, cursor_char, &t);
                        changed = true;
                    }
                }
                egui::Event::Paste(t) => {
                    insert_text_at_cursor(input, cursor_char, &t.replace(['\r', '\n'], " "));
                    changed = true;
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => match key {
                    egui::Key::Enter => {
                        match enter_key_action(shortcode_menu_open, modifiers.shift) {
                            EnterKeyAction::InsertNewline => {
                                insert_text_at_cursor(input, cursor_char, "\n");
                                changed = true;
                            }
                            EnterKeyAction::AcceptShortcode => {
                                changed |= accept_selected_shortcode(
                                    input,
                                    cursor_char,
                                    emoji_alias_to_char,
                                    emoji_aliases,
                                    shortcode_selected,
                                );
                            }
                            EnterKeyAction::Submit => {
                                submit = true;
                            }
                        }
                    }
                    egui::Key::Tab => {
                        let suggestions = crate::ui::emoji_picker::shortcode_suggestions(
                            input,
                            *cursor_char,
                            emoji_alias_to_char,
                            emoji_aliases,
                            1,
                        );
                        if let Some((_alias, ch)) = suggestions.first() {
                            if let Some((start, _query)) =
                                crate::ui::emoji_picker::emoji_shortcode_trigger(
                                    input,
                                    *cursor_char,
                                )
                            {
                                replace_char_range(input, cursor_char, start, *cursor_char, ch);
                                changed = true;
                            }
                        }
                    }
                    egui::Key::Backspace => {
                        let before = input.len();
                        remove_prev_char(input, cursor_char);
                        changed |= input.len() != before;
                    }
                    egui::Key::Delete => {
                        let before = input.len();
                        remove_next_char(input, cursor_char);
                        changed |= input.len() != before;
                    }
                    egui::Key::ArrowLeft => {
                        if *cursor_char > 0 {
                            *cursor_char -= 1;
                        }
                    }
                    egui::Key::ArrowRight => {
                        let len = input.chars().count();
                        if *cursor_char < len {
                            *cursor_char += 1;
                        }
                    }
                    egui::Key::ArrowUp => {
                        if !shortcode_menu_open {
                            let points = composer_caret_positions(
                                ui,
                                input,
                                emoji_map,
                                18.0,
                                content_rect.width().max(20.0),
                            );
                            move_cursor_vertical(&points, cursor_char, -1, line_height);
                        }
                    }
                    egui::Key::ArrowDown => {
                        if !shortcode_menu_open {
                            let points = composer_caret_positions(
                                ui,
                                input,
                                emoji_map,
                                18.0,
                                content_rect.width().max(20.0),
                            );
                            move_cursor_vertical(&points, cursor_char, 1, line_height);
                        }
                    }
                    egui::Key::Home => {
                        *cursor_char = 0;
                    }
                    egui::Key::End => {
                        *cursor_char = input.chars().count();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    let frame_fill = egui::Color32::TRANSPARENT;
    let frame_stroke = egui::Stroke::NONE;

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(12),
        frame_fill,
        frame_stroke,
        egui::StrokeKind::Outside,
    );

    if input.is_empty() {
        ui.painter().text(
            content_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            "Send a message...",
            egui::TextStyle::Body.resolve(ui.style()),
            egui::Color32::from_rgb(185, 187, 192),
        );
    } else {
        let painter = ui.painter().with_clip_rect(content_rect);
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;
        let mut x = content_rect.left();
        let right = content_rect.right();
        let scroll_px = *scroll_lines * line_height;
        let mut y = content_rect.top() + 11.0 - scroll_px;

        while i < chars.len() {
            if chars[i] == '\n' {
                x = content_rect.left();
                y += line_height;
                i += 1;
                continue;
            }

            let mut matched = false;
            for len in [2usize, 1usize] {
                if i + len <= chars.len() {
                    let s: String = chars[i..i + len].iter().collect();
                    if let Some(&idx) = emoji_map.get(&s) {
                        if let Some((_, tex)) = emoji_textures.get(idx) {
                            if x + 20.0 > right && x > content_rect.left() {
                                x = content_rect.left();
                                y += line_height;
                            }
                            let img_rect = egui::Rect::from_min_size(
                                egui::pos2(x, y - 9.0),
                                egui::vec2(18.0, 18.0),
                            );
                            painter.image(
                                tex.id(),
                                img_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                            x += 20.0;
                        }
                        i += len;
                        matched = true;
                        break;
                    }
                }
            }

            if !matched {
                let glyph = chars[i].to_string();
                let glyph_w = measure_text_width(ui, &glyph);
                if x + glyph_w > right && x > content_rect.left() {
                    x = content_rect.left();
                    y += line_height;
                }
                painter.text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    &glyph,
                    egui::TextStyle::Body.resolve(ui.style()),
                    egui::Color32::from_rgb(244, 245, 247),
                );
                x += glyph_w;
                i += 1;
            }
        }

        if needs_scrollbar {
            let track = egui::Rect::from_min_max(
                egui::pos2(rect.right() - 8.0, content_rect.top()),
                egui::pos2(rect.right() - 4.0, content_rect.bottom()),
            );
            ui.painter()
                .rect_filled(track, 2.0, ui.visuals().widgets.noninteractive.bg_fill);

            let thumb_h = (track.height() * (visual_lines / line_count as f32)).max(18.0);
            let travel = (track.height() - thumb_h).max(0.0);
            let t = if max_scroll <= 0.0 {
                0.0
            } else {
                *scroll_lines / max_scroll
            };
            let thumb_top = track.top() + travel * t;
            let thumb = egui::Rect::from_min_max(
                egui::pos2(track.left(), thumb_top),
                egui::pos2(track.right(), thumb_top + thumb_h),
            );
            ui.painter().rect_filled(
                thumb,
                2.0,
                ui.visuals().widgets.active.bg_fill.gamma_multiply(0.9),
            );

            let scroll_id = response.id.with("scrollbar");
            let scroll_resp = ui.interact(track, scroll_id, egui::Sense::click_and_drag());
            if (scroll_resp.clicked() || scroll_resp.dragged()) && max_scroll > 0.0 {
                if let Some(pos) = scroll_resp.interact_pointer_pos() {
                    let rel = ((pos.y - track.top()) / track.height()).clamp(0.0, 1.0);
                    *scroll_lines = rel * max_scroll;
                }
            }
        }
    }

    if has_focus {
        let blink_on = ((ui.input(|i| i.time) * 2.0) as i64) % 2 == 0;
        if blink_on {
            let caret = caret_points
                .get(*cursor_char)
                .copied()
                .unwrap_or_else(|| *caret_points.last().unwrap_or(&egui::pos2(0.0, 0.0)));
            let x = content_rect.left() + caret.x + 1.0;
            let top = content_rect.top() + caret.y + 2.0 - (*scroll_lines * line_height);
            let bottom = (top + 18.0).min(content_rect.bottom() - 2.0);
            if top < content_rect.bottom() && bottom > content_rect.top() {
                ui.painter().line_segment(
                    [
                        egui::pos2(x, top.max(content_rect.top())),
                        egui::pos2(x, bottom),
                    ],
                    egui::Stroke::new(1.6, egui::Color32::from_rgb(250, 250, 252)),
                );
            }
        }
    }

    (response, submit, changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn emoji_index() -> (HashMap<String, String>, Vec<String>) {
        let mut alias_to_char = HashMap::new();
        alias_to_char.insert("joy".to_string(), "😂".to_string());
        alias_to_char.insert("joy_cat".to_string(), "😹".to_string());
        alias_to_char.insert("smile".to_string(), "😊".to_string());
        let aliases = vec![
            "joy".to_string(),
            "joy_cat".to_string(),
            "smile".to_string(),
        ];
        (alias_to_char, aliases)
    }

    #[test]
    fn enter_with_shortcode_menu_accepts_selection_instead_of_submit() {
        assert_eq!(
            enter_key_action(true, false),
            EnterKeyAction::AcceptShortcode
        );
    }

    #[test]
    fn enter_without_shortcode_menu_submits_message() {
        assert_eq!(enter_key_action(false, false), EnterKeyAction::Submit);
    }

    #[test]
    fn shift_enter_inserts_newline_even_when_shortcode_menu_is_open() {
        assert_eq!(enter_key_action(true, true), EnterKeyAction::InsertNewline);
    }

    #[test]
    fn accept_selected_shortcode_replaces_query_for_enter_without_adding_space() {
        let (alias_to_char, aliases) = emoji_index();
        let mut input = "hello :jo".to_string();
        let mut cursor = input.chars().count();

        let accepted =
            accept_selected_shortcode(&mut input, &mut cursor, &alias_to_char, &aliases, 0);

        assert!(accepted);
        assert_eq!(input, "hello 😂");
        assert_eq!(cursor, input.chars().count());
    }

    #[test]
    fn regular_space_does_not_accept_shortcode() {
        let (alias_to_char, aliases) = emoji_index();
        let mut input = "hello :jo".to_string();
        let mut cursor = input.chars().count();

        insert_text_at_cursor(&mut input, &mut cursor, " ");

        assert_eq!(input, "hello :jo ");
        assert_eq!(cursor, input.chars().count());
        let suggestions = crate::ui::emoji_picker::shortcode_suggestions(
            &input,
            cursor,
            &alias_to_char,
            &aliases,
            10,
        );
        assert!(suggestions.is_empty());
    }

    #[test]
    fn accept_selected_shortcode_uses_highlighted_suggestion() {
        let (alias_to_char, aliases) = emoji_index();
        let mut input = "hello :jo".to_string();
        let mut cursor = input.chars().count();

        let accepted =
            accept_selected_shortcode(&mut input, &mut cursor, &alias_to_char, &aliases, 1);

        assert!(accepted);
        assert_eq!(input, "hello 😹");
        assert_eq!(cursor, input.chars().count());
    }
}
