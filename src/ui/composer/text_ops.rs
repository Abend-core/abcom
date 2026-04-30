use eframe::egui;

/// Convertit un index de caractère (unicode) en index d'octet dans la string
pub fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

/// Insère du texte à la position curseur (char index)
pub fn insert_text_at_cursor(text: &mut String, cursor: &mut usize, to_insert: &str) {
    let byte_idx = char_to_byte_idx(text, *cursor);
    text.insert_str(byte_idx, to_insert);
    *cursor += to_insert.chars().count();
}

/// Supprime le caractère avant le curseur (Backspace)
pub fn remove_prev_char(text: &mut String, cursor: &mut usize) {
    if *cursor == 0 { return; }
    let start_byte = char_to_byte_idx(text, *cursor - 1);
    let end_byte = char_to_byte_idx(text, *cursor);
    text.replace_range(start_byte..end_byte, "");
    *cursor -= 1;
}

/// Supprime le caractère après le curseur (Delete)
pub fn remove_next_char(text: &mut String, cursor: &mut usize) {
    let total = text.chars().count();
    if *cursor >= total { return; }
    let start_byte = char_to_byte_idx(text, *cursor);
    let end_byte = char_to_byte_idx(text, *cursor + 1);
    text.replace_range(start_byte..end_byte, "");
}

/// Insère un emoji à la position curseur
pub fn insert_emoji_at_cursor(text: &mut String, cursor: &mut usize, emoji: &str) {
    insert_text_at_cursor(text, cursor, emoji);
}

/// Remplace la plage [start_char, end_char) par `replacement`
pub fn replace_char_range(text: &mut String, cursor: &mut usize, start: usize, end: usize, replacement: &str) {
    let byte_start = char_to_byte_idx(text, start);
    let byte_end = char_to_byte_idx(text, end);
    text.replace_range(byte_start..byte_end, replacement);
    *cursor = start + replacement.chars().count();
}
