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

/// Début du mot précédant le curseur : saute d'abord les blancs, puis le mot.
/// Utilisé pour Option/Ctrl+Backspace et Option/Ctrl+Flèche gauche.
pub fn prev_word_start(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = cursor.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

/// Fin du mot suivant le curseur : saute d'abord les blancs, puis le mot.
/// Utilisé pour Option/Ctrl+Delete et Option/Ctrl+Flèche droite.
pub fn next_word_end(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let mut i = cursor.min(total);
    while i < total && chars[i].is_whitespace() {
        i += 1;
    }
    while i < total && !chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// Début de la ligne visuelle contenant le curseur (après le `\n` précédent).
pub fn line_start(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut i = cursor.min(chars.len());
    while i > 0 && chars[i - 1] != '\n' {
        i -= 1;
    }
    i
}

/// Fin de la ligne contenant le curseur (avant le `\n` suivant).
pub fn line_end(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let mut i = cursor.min(total);
    while i < total && chars[i] != '\n' {
        i += 1;
    }
    i
}

/// Extrait la sous-chaîne couvrant la plage de caractères [start, end).
pub fn char_range_string(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

/// Insère un emoji à la position curseur
pub fn insert_emoji_at_cursor(text: &mut String, cursor: &mut usize, emoji: &str) {
    insert_text_at_cursor(text, cursor, emoji);
}

/// Préfixe de `text` limité à `max_chars` caractères Unicode, sans jamais
/// couper au milieu d'un point de code UTF-8 (accents et emoji comptent
/// chacun pour un caractère).
pub fn char_prefix(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => &text[..byte_idx],
        None => text,
    }
}

/// Remplace la plage [start_char, end_char) par `replacement`
pub fn replace_char_range(
    text: &mut String,
    cursor: &mut usize,
    start: usize,
    end: usize,
    replacement: &str,
) {
    let byte_start = char_to_byte_idx(text, start);
    let byte_end = char_to_byte_idx(text, end);
    text.replace_range(byte_start..byte_end, replacement);
    *cursor = start + replacement.chars().count();
}

#[cfg(test)]
#[path = "../../tests/test_ui_composer_text_ops.rs"]
mod tests;
