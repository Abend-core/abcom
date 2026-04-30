/// Détecte si le curseur est dans un shortcode `:xxx`
/// Retourne (start_char_index_of_colon, query_after_colon) si c'est le cas
pub fn emoji_shortcode_trigger(input: &str, cursor_char: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let end = cursor_char.min(chars.len());
    let slice: String = chars[..end].iter().collect();
    if let Some(colon_byte) = slice.rfind(':') {
        let after = &slice[colon_byte + 1..];
        if !after.contains(' ') && !after.contains(':') {
            let start_char = slice[..colon_byte].chars().count();
            return Some((start_char, after.to_string()));
        }
    }
    None
}

/// Retourne jusqu'à `limit` suggestions de shortcodes correspondant à la requête courante
pub fn shortcode_suggestions(
    input: &str,
    cursor_char: usize,
    alias_to_char: &std::collections::HashMap<String, String>,
    aliases: &[String],
    limit: usize,
) -> Vec<(String, String)> {
    let Some((_start, query)) = emoji_shortcode_trigger(input, cursor_char) else {
        return Vec::new();
    };
    if query.is_empty() { return Vec::new(); }
    aliases.iter()
        .filter(|a| a.starts_with(&query))
        .take(limit)
        .filter_map(|a| alias_to_char.get(a).map(|ch| (a.clone(), ch.clone())))
        .collect()
}
