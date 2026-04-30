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
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // \u2500\u2500 emoji_shortcode_trigger \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500

    #[test]
    fn trigger_no_colon() {
        assert!(emoji_shortcode_trigger("hello world", 11).is_none());
    }

    #[test]
    fn trigger_colon_with_query() {
        let r = emoji_shortcode_trigger(":smi", 4);
        assert_eq!(r, Some((0, "smi".to_string())));
    }

    #[test]
    fn trigger_colon_with_space_after() {
        // Space after colon \u2192 not a shortcode
        assert!(emoji_shortcode_trigger(": hello", 7).is_none());
    }

    #[test]
    fn trigger_double_colon_completes() {
        // ":smile:" \u2192 colon_pos found is the second colon, but "after" contains ""
        // Actually rfind finds the LAST colon which is at position 6
        // after = "" \u2192 no space, no colon \u2192 Some((6, ""))
        let r = emoji_shortcode_trigger(":smile:", 7);
        assert!(r.is_some()); // second colon starts a new trigger
    }

    #[test]
    fn trigger_cursor_at_start() {
        // cursor=0 \u2192 slice is empty, no colon
        assert!(emoji_shortcode_trigger(":hello", 0).is_none());
    }

    #[test]
    fn trigger_colon_start_position() {
        // "hi :smi" \u2192 colon at char 3
        let r = emoji_shortcode_trigger("hi :smi", 7);
        assert_eq!(r, Some((3, "smi".to_string())));
    }

    #[test]
    fn trigger_empty_query_at_colon() {
        // Cursor right after colon \u2192 empty query
        let r = emoji_shortcode_trigger("hi :", 4);
        assert_eq!(r, Some((3, "".to_string())));
    }

    // \u2500\u2500 shortcode_suggestions \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500

    fn make_index() -> (HashMap<String, String>, Vec<String>) {
        let mut m = HashMap::new();
        m.insert("smile".to_string(), "😊".to_string());
        m.insert("smirk".to_string(), "😏".to_string());
        m.insert("heart".to_string(), "❤️".to_string());
        m.insert("sob".to_string(), "😭".to_string());
        let mut aliases: Vec<String> = m.keys().cloned().collect();
        aliases.sort();
        (m, aliases)
    }

    #[test]
    fn suggestions_no_trigger_returns_empty() {
        let (m, aliases) = make_index();
        let result = shortcode_suggestions("no colon here", 13, &m, &aliases, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn suggestions_empty_query_returns_empty() {
        let (m, aliases) = make_index();
        // cursor right after colon \u2192 empty query \u2192 no suggestions (guard in function)
        let result = shortcode_suggestions(":", 1, &m, &aliases, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn suggestions_prefix_match() {
        let (m, aliases) = make_index();
        let result = shortcode_suggestions(":smi", 4, &m, &aliases, 10);
        assert_eq!(result.len(), 2); // smile + smirk
        let names: Vec<&str> = result.iter().map(|(a, _)| a.as_str()).collect();
        assert!(names.contains(&"smile"));
        assert!(names.contains(&"smirk"));
    }

    #[test]
    fn suggestions_exact_match() {
        let (m, aliases) = make_index();
        let result = shortcode_suggestions(":heart", 6, &m, &aliases, 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "heart");
        assert_eq!(result[0].1, "❤️");
    }

    #[test]
    fn suggestions_respects_limit() {
        let (m, aliases) = make_index();
        // "s" matches smile, smirk, sob \u2192 3 matches, limit to 2
        let result = shortcode_suggestions(":s", 2, &m, &aliases, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn suggestions_no_match_returns_empty() {
        let (m, aliases) = make_index();
        let result = shortcode_suggestions(":xyz", 4, &m, &aliases, 10);
        assert!(result.is_empty());
    }
}