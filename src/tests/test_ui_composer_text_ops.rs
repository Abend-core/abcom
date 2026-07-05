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

// ── remove_prev_char ─────────────────────────────────────────

#[test]
fn remove_prev_normal() {
    let mut t = "hello".to_string();
    let mut c = 3;
    remove_prev_char(&mut t, &mut c);
    assert_eq!(t, "helo");
    assert_eq!(c, 2);
}

#[test]
fn remove_prev_at_zero_noop() {
    let mut t = "hello".to_string();
    let mut c = 0;
    remove_prev_char(&mut t, &mut c);
    assert_eq!(t, "hello");
    assert_eq!(c, 0);
}

#[test]
fn remove_prev_multibyte() {
    let mut t = "café".to_string();
    let total = t.chars().count(); // 4
    let mut c = total;
    remove_prev_char(&mut t, &mut c);
    assert_eq!(t, "caf");
    assert_eq!(c, 3);
}

// ── remove_next_char ─────────────────────────────────────────

#[test]
fn remove_next_normal() {
    let mut t = "hello".to_string();
    let mut c = 1;
    remove_next_char(&mut t, &mut c);
    assert_eq!(t, "hllo");
    assert_eq!(c, 1);
}

#[test]
fn remove_next_at_end_noop() {
    let mut t = "hello".to_string();
    let mut c = 5;
    remove_next_char(&mut t, &mut c);
    assert_eq!(t, "hello");
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
