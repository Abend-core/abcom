use eframe::egui;

use super::render::measure_text_width;

/// Retourne les positions pixel (x, y) de chaque caractère dans le composeur.
/// Retourne aussi la liste (line_start_char_idx, y_pixel) pour chaque ligne.
pub fn composer_caret_positions(
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    _emoji_textures: &[(String, egui::TextureHandle)],
    ui: &egui::Ui,
    line_height: f32,
) -> (Vec<(usize, f32)>, Vec<(usize, f32)>) {
    let inner_rect = ui.available_rect_before_wrap();
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let chars: Vec<char> = text.chars().collect();

    let mut positions = Vec::with_capacity(chars.len() + 1);
    let mut line_starts = Vec::new();

    let mut x = inner_rect.left();
    let mut y = inner_rect.top();
    let mut i = 0usize;
    line_starts.push((0, y));

    while i < chars.len() {
        let ch = chars[i];
        let mut drawn = false;
        for len in [2usize, 1usize] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if emoji_map.contains_key(&s) {
                    if x + line_height > inner_rect.right() && x > inner_rect.left() {
                        x = inner_rect.left();
                        y += line_height;
                        line_starts.push((i, y));
                    }
                    positions.push((i, x));
                    x += line_height;
                    i += len;
                    drawn = true;
                    break;
                }
            }
        }
        if !drawn {
            if ch == '\u{fe0f}' || ch == '\u{200d}' {
                i += 1;
                continue;
            }
            if ch == '\n' {
                positions.push((i, x));
                x = inner_rect.left();
                y += line_height;
                i += 1;
                line_starts.push((i, y));
                continue;
            }
            let glyph: String = std::iter::once(ch).collect();
            let glyph_w = measure_text_width(&glyph, ui.ctx(), &font_id);
            if x + glyph_w > inner_rect.right() && x > inner_rect.left() {
                x = inner_rect.left();
                y += line_height;
                line_starts.push((i, y));
            }
            positions.push((i, x));
            x += glyph_w;
            i += 1;
        }
    }
    positions.push((chars.len(), x));

    (line_starts, positions)
}

/// Calcule le curseur (char index) à partir d'un point pixel dans le composeur
#[allow(dead_code)]
pub fn cursor_from_point(
    pos: egui::Pos2,
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    _emoji_textures: &[(String, egui::TextureHandle)],
    ui: &egui::Ui,
    _line_height: f32,
) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let (line_starts, positions) =
        composer_caret_positions(text, emoji_map, _emoji_textures, ui, _line_height);

    let line_idx = line_starts
        .iter()
        .enumerate()
        .rev()
        .find(|(_, (_, y))| pos.y >= *y)
        .map(|(i, _)| i)
        .unwrap_or(0);

    let (line_start_char, _) = line_starts[line_idx];
    let line_end_char = line_starts
        .get(line_idx + 1)
        .map(|(c, _)| *c)
        .unwrap_or(chars.len());

    let line_positions: Vec<(usize, f32)> = positions
        .iter()
        .cloned()
        .filter(|(idx, _)| *idx >= line_start_char && *idx < line_end_char)
        .collect();

    if line_positions.is_empty() {
        return line_start_char.min(chars.len());
    }

    line_positions
        .iter()
        .min_by(|(_, x1), (_, x2)| {
            let d1 = (*x1 - pos.x).abs();
            let d2 = (*x2 - pos.x).abs();
            d1.partial_cmp(&d2).unwrap()
        })
        .map(|(idx, _)| *idx)
        .unwrap_or(chars.len())
}

/// Déplace le curseur verticalement (direction: -1 = haut, +1 = bas)
pub fn move_cursor_vertical(
    text: &str,
    cursor: usize,
    direction: i32,
    _origin_x: f32,
    _origin_y: f32,
    _line_height: f32,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
    ui: &egui::Ui,
) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let (line_starts, positions) =
        composer_caret_positions(text, emoji_map, emoji_textures, ui, _line_height);

    let line_idx = line_starts
        .iter()
        .enumerate()
        .rev()
        .find(|(_, (start, _))| *start <= cursor)
        .map(|(i, _)| i)
        .unwrap_or(0);

    let current_x = positions
        .iter()
        .find(|(idx, _)| *idx == cursor)
        .map(|(_, x)| *x)
        .or_else(|| {
            positions
                .iter()
                .rev()
                .find(|(idx, _)| *idx < cursor)
                .map(|(_, x)| *x)
        })
        .unwrap_or(line_starts[line_idx].1);

    let target_line = if direction < 0 {
        if line_idx == 0 {
            return cursor;
        }
        line_idx - 1
    } else {
        if line_idx + 1 >= line_starts.len() {
            return cursor;
        }
        line_idx + 1
    };

    let (target_start, _) = line_starts[target_line];
    let target_end = line_starts
        .get(target_line + 1)
        .map(|(c, _)| *c)
        .unwrap_or(chars.len());

    let target_positions: Vec<(usize, f32)> = positions
        .iter()
        .cloned()
        .filter(|(idx, _)| *idx >= target_start && *idx < target_end)
        .collect();

    if target_positions.is_empty() {
        return target_start.min(chars.len());
    }

    // Keep the same visual column: choose the first position at or after the target x.
    for (idx, x) in &target_positions {
        if *x >= current_x {
            return *idx;
        }
    }

    target_positions
        .last()
        .map(|(idx, _)| *idx)
        .unwrap_or(target_start.min(chars.len()))
}
