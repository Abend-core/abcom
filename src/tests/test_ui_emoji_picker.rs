use super::emoji_shortcode_trigger;

#[test]
fn trigger_detects_query_after_colon() {
    assert_eq!(
        emoji_shortcode_trigger("hello :jo", 9),
        Some((6, "jo".to_string()))
    );
}

#[test]
fn trigger_returns_empty_query_right_after_colon() {
    assert_eq!(emoji_shortcode_trigger(":", 1), Some((0, String::new())));
}

#[test]
fn trigger_ignores_plain_text() {
    assert_eq!(emoji_shortcode_trigger("hello", 5), None);
}

/// Régression : curseur placé juste AVANT un `:` (début de texte, après un
/// espace ou un saut de ligne). `start == cursor_char` faisait paniquer la
/// slice `chars[start + 1..cursor_char]` — crash de l'application en release
/// (panic = abort), notamment après un Shift+Entrée devant un shortcode.
#[test]
fn trigger_does_not_panic_with_cursor_before_colon() {
    assert_eq!(emoji_shortcode_trigger(":jo", 0), None);
    assert_eq!(emoji_shortcode_trigger("salut :jo", 6), None);
    assert_eq!(emoji_shortcode_trigger("salut\n:jo", 6), None);
}

#[test]
fn trigger_stops_at_newline_like_whitespace() {
    assert_eq!(
        emoji_shortcode_trigger("salut\n:jo", 9),
        Some((6, "jo".to_string()))
    );
}
