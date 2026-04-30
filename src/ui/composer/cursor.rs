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
                    positions.push((i, x));
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
                positions.push((i, x));
                x = inner_rect.left();
                y += line_height;
                i += 1;
                line_starts.push((i, y));
                continue;
            }
            positions.push((i, x));
            let glyph: String = std::iter::once(ch).collect();
            x += measure_text_width(&glyph, ui.ctx(), &font_id);
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
    let (line_starts, positions) = composer_caret_positions(text, emoji_map, _emoji_textures, ui, _line_height);

    // Find line
    let line_idx = line_starts.iter().enumerate()
        .rev()
        .find(|(_, (_, y))| pos.y >= *y)
        .map(|(i, _)| i)
        .unwrap_or(0);

    let (line_start_char, _) = line_starts[line_idx];
    let line_end_char = line_starts.get(line_idx + 1).map(|(c, _)| *c).unwrap_or(positions.len());

    // Find closest in line
    positions[line_start_char..line_end_char.min(positions.len())]
        .iter()
        .enumerate()
        .min_by_key(|(_, (_, x))| ((*x - pos.x).abs() * 1000.0) as i64)
        .map(|(i, _)| line_start_char + i)
        .unwrap_or(text.chars().count())
}

/// Déplace le curseur verticalement (direction: -1 = haut, +1 = bas)
pub fn move_cursor_vertical(
    text: &str,
    cursor: usize,
    direction: i32,
    _origin_x: f32,
    _origin_y: f32,
    _line_height: f32,
    _emoji_map: &std::collections::HashMap<String, usize>,
    _emoji_textures: &[(String, egui::TextureHandle)],
    _ctx: &egui::Context,
) -> usize {
    // Simple approach: find current line, move up/down by one line
    let chars: Vec<char> = text.chars().collect();
    let current_line_start = {
        let mut i = cursor;
        while i > 0 && chars[i - 1] != '\n' { i -= 1; }
        i
    };
    let current_line_end = {
        let mut i = cursor;
        while i < chars.len() && chars[i] != '\n' { i += 1; }
        i
    };
    let col_in_line = cursor - current_line_start;

    if direction < 0 {
        // Move up: find prev line
        if current_line_start == 0 { return 0; }
        let prev_line_end = current_line_start - 1;
        let prev_line_start = {
            let mut i = prev_line_end;
            while i > 0 && chars[i - 1] != '\n' { i -= 1; }
            i
        };
        let prev_line_len = prev_line_end - prev_line_start;
        prev_line_start + col_in_line.min(prev_line_len)
    } else {
        // Move down: find next line
        if current_line_end >= chars.len() { return chars.len(); }
        let next_line_start = current_line_end + 1;
        let next_line_end = {
            let mut i = next_line_start;
            while i < chars.len() && chars[i] != '\n' { i += 1; }
            i
        };
        let next_line_len = next_line_end - next_line_start;
        next_line_start + col_in_line.min(next_line_len)
    }
}
