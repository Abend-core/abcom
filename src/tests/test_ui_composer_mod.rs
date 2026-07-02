
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

    let accepted = accept_selected_shortcode(&mut input, &mut cursor, &alias_to_char, &aliases, 0);

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

    let accepted = accept_selected_shortcode(&mut input, &mut cursor, &alias_to_char, &aliases, 1);

    assert!(accepted);
    assert_eq!(input, "hello 😹");
    assert_eq!(cursor, input.chars().count());
}
