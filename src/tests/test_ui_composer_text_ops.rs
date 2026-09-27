use super::*;

// ── char_to_byte_idx ──────────────────────────────────────────

#[test]
fn byte_idx_ascii() {
    let s = "hello";
    assert_eq!(char_to_byte_idx(s, 0), 0);
    assert_eq!(char_to_byte_idx(s, 3), 3);
    assert_eq!(char_to_byte_idx(s, 5), 5); // beyond end
}

#[test]
fn byte_idx_multibyte() {
    // 'é' is 2 bytes in UTF-8
    let s = "héllo";
    assert_eq!(char_to_byte_idx(s, 1), 1); // 'h'
    assert_eq!(char_to_byte_idx(s, 2), 3); // after 'é' (2 bytes)
    assert_eq!(char_to_byte_idx(s, 3), 4);
}

#[test]
fn byte_idx_emoji() {
    // 🎉 is 4 bytes
    let s = "a🎉b";
    assert_eq!(char_to_byte_idx(s, 0), 0);
    assert_eq!(char_to_byte_idx(s, 1), 1);
    assert_eq!(char_to_byte_idx(s, 2), 5); // after emoji
    assert_eq!(char_to_byte_idx(s, 3), 6);
}

#[test]
fn byte_idx_overflow_returns_len() {
    let s = "abc";
    assert_eq!(char_to_byte_idx(s, 99), 3);
}

// ── insert_text_at_cursor ─────────────────────────────────────

#[test]
fn insert_at_start() {
    let mut t = "world".to_string();
    let mut c = 0;
    insert_text_at_cursor(&mut t, &mut c, "hello ");
    assert_eq!(t, "hello world");
    assert_eq!(c, 6);
}

#[test]
fn insert_at_end() {
    let mut t = "hello".to_string();
    let mut c = 5;
    insert_text_at_cursor(&mut t, &mut c, " world");
    assert_eq!(t, "hello world");
    assert_eq!(c, 11);
}

#[test]
fn insert_in_middle() {
    let mut t = "hllo".to_string();
    let mut c = 1;
    insert_text_at_cursor(&mut t, &mut c, "e");
    assert_eq!(t, "hello");
    assert_eq!(c, 2);
}

#[test]
fn insert_emoji_moves_cursor() {
    let mut t = "hi".to_string();
    let mut c = 2;
    insert_emoji_at_cursor(&mut t, &mut c, "🎉");
    assert_eq!(t, "hi🎉");
    assert_eq!(c, 3); // 1 char
}

// ── prev_word_start / next_word_end ──────────────────────────

#[test]
fn prev_word_start_skips_trailing_spaces_then_word() {
    assert_eq!(prev_word_start("hello world", 11), 6);
    assert_eq!(prev_word_start("hello world  ", 13), 6);
    assert_eq!(prev_word_start("hello", 3), 0);
    assert_eq!(prev_word_start("hello", 0), 0);
}

#[test]
fn next_word_end_skips_leading_spaces_then_word() {
    assert_eq!(next_word_end("hello world", 0), 5);
    assert_eq!(next_word_end("hello world", 5), 11);
    assert_eq!(next_word_end("hello", 5), 5);
}

// ── line_start / line_end ─────────────────────────────────────

#[test]
fn line_bounds_with_newlines() {
    let t = "salut\nles amis";
    assert_eq!(line_start(t, 14), 6);
    assert_eq!(line_start(t, 3), 0);
    assert_eq!(line_end(t, 0), 5);
    assert_eq!(line_end(t, 6), 14);
}

// ── char_range_string ─────────────────────────────────────────

#[test]
fn char_range_string_handles_multibyte_and_inverted_range() {
    assert_eq!(char_range_string("a🎉b", 1, 2), "🎉");
    assert_eq!(char_range_string("hello", 1, 4), "ell");
    assert_eq!(char_range_string("hello", 4, 1), "");
}

// ── replace_char_range ────────────────────────────────────────

#[test]
fn replace_range_basic() {
    let mut t = ":thumbsup:".to_string();
    let mut c = 10;
    replace_char_range(&mut t, &mut c, 0, 10, "👍");
    assert_eq!(t, "👍");
    assert_eq!(c, 1);
}

#[test]
fn replace_range_shortcode_to_emoji() {
    let mut t = "je :smile le".to_string();
    // Replace ':smile' (chars 3..9) with '😊'
    let mut c = 9;
    replace_char_range(&mut t, &mut c, 3, 9, "😊");
    assert_eq!(t, "je 😊 le");
    assert_eq!(c, 4);
}

// ── char_prefix ───────────────────────────────────────────────

#[test]
fn char_prefix_ascii() {
    assert_eq!(char_prefix("bonjour", 3), "bon");
    assert_eq!(char_prefix("bonjour", 0), "");
}

#[test]
fn char_prefix_shorter_than_max_returns_all() {
    assert_eq!(char_prefix("salut", 100), "salut");
}

#[test]
fn char_prefix_counts_unicode_chars_not_bytes() {
    // « é » (2 octets) et emoji (4 octets) comptent pour un caractère,
    // et la coupe ne tombe jamais au milieu d'un point de code UTF-8.
    assert_eq!(char_prefix("héhé", 2), "hé");
    assert_eq!(char_prefix("a😀b😀c", 2), "a😀");
    assert_eq!(char_prefix("ééé", 1), "é");
}
