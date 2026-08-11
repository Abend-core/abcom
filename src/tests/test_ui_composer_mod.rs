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

const SHIFT: egui::Modifiers = egui::Modifiers {
    shift: true,
    alt: false,
    ctrl: false,
    mac_cmd: false,
    command: false,
};

#[test]
fn enter_with_shortcode_menu_accepts_selection_instead_of_submit() {
    assert_eq!(
        enter_key_action(true, egui::Modifiers::NONE),
        EnterKeyAction::AcceptShortcode
    );
}

#[test]
fn plain_enter_inserts_newline() {
    assert_eq!(
        enter_key_action(false, egui::Modifiers::NONE),
        EnterKeyAction::InsertNewline
    );
}

#[test]
fn shift_enter_inserts_newline_even_when_shortcode_menu_is_open() {
    assert_eq!(enter_key_action(true, SHIFT), EnterKeyAction::InsertNewline);
}

#[test]
fn command_or_ctrl_enter_submits_message() {
    assert_eq!(
        enter_key_action(false, egui::Modifiers::COMMAND),
        EnterKeyAction::Submit
    );
    assert_eq!(
        enter_key_action(false, egui::Modifiers::CTRL),
        EnterKeyAction::Submit
    );
    // Même menu ouvert : l'envoi explicite garde la priorité.
    assert_eq!(
        enter_key_action(true, egui::Modifiers::COMMAND),
        EnterKeyAction::Submit
    );
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

/// Pilote `custom_composer_input` dans un contexte egui headless : injecte des
/// événements clavier et rend plusieurs frames comme le ferait l'application.
fn run_composer_frames(
    input: &mut String,
    cursor: &mut usize,
    frames: Vec<Vec<egui::Event>>,
) -> bool {
    run_composer_session(input, cursor, frames, false).0
}

/// Variante exposant le menu de shortcodes et le défilement final.
fn run_composer_session(
    input: &mut String,
    cursor: &mut usize,
    frames: Vec<Vec<egui::Event>>,
    shortcode_menu_open: bool,
) -> (bool, f32) {
    let ctx = egui::Context::default();
    let mut has_focus = true;
    let mut scroll = 0.0f32;
    let mut anchor = None;
    let emoji_map = HashMap::new();
    let textures = crate::ui::EmojiTextures::default();
    let (alias_to_char, aliases) = emoji_index();
    let mut submitted = false;

    for events in frames {
        let raw = egui::RawInput {
            events,
            ..Default::default()
        };
        // epaint 0.36 panique si les deltas de textures d'une frame sont
        // abandonnés : ici on ne peint rien, on les libère explicitement.
        let mut output = ctx.run_ui(raw, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                let (_, submit, _, _) = custom_composer_input(
                    ui,
                    input,
                    cursor,
                    &mut has_focus,
                    &mut scroll,
                    &emoji_map,
                    &textures,
                    &alias_to_char,
                    &aliases,
                    shortcode_menu_open,
                    0,
                    300.0,
                    &mut anchor,
                );
                submitted |= submit;
            });
        });
        output.textures_delta.clear();
    }
    (submitted, scroll)
}

fn key_event(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn shift_enter_inserts_newline_and_next_frame_renders_without_panic() {
    let mut input = "hello".to_string();
    let mut cursor = input.chars().count();

    let submitted = run_composer_frames(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::Enter, SHIFT)], vec![], vec![]],
    );

    assert!(!submitted);
    assert_eq!(input, "hello\n");
    assert_eq!(cursor, input.chars().count());
}

#[test]
fn plain_enter_inserts_newline_instead_of_submitting() {
    let mut input = "hello".to_string();
    let mut cursor = input.chars().count();

    let submitted = run_composer_frames(
        &mut input,
        &mut cursor,
        vec![
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            vec![],
        ],
    );

    assert!(!submitted);
    assert_eq!(input, "hello\n");
}

#[test]
fn command_enter_submits_without_touching_text() {
    let mut input = "hello".to_string();
    let mut cursor = input.chars().count();

    let submitted = run_composer_frames(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::Enter, egui::Modifiers::COMMAND)]],
    );

    assert!(submitted);
    assert_eq!(input, "hello");
}

/// Régression : saisie et création de sélection dans la MÊME frame (rafale
/// clavier, répétition avec Maj). Les positions de caractères étaient
/// calculées avant le traitement des événements et la peinture de la
/// sélection lisait des points périmés → « index out of bounds: the len is 1
/// but the index is 1 » (abort en release).
#[test]
fn same_frame_typing_and_selection_does_not_panic() {
    let mut input = String::new();
    let mut cursor = 0usize;

    run_composer_frames(
        &mut input,
        &mut cursor,
        vec![
            vec![
                egui::Event::Text("a".to_string()),
                key_event(egui::Key::ArrowLeft, SHIFT),
            ],
            vec![],
        ],
    );

    assert_eq!(input, "a");
    assert_eq!(cursor, 0);
}

#[test]
fn alt_backspace_deletes_previous_word() {
    let mut input = "hello world".to_string();
    let mut cursor = input.chars().count();
    let alt = egui::Modifiers {
        alt: true,
        ..egui::Modifiers::NONE
    };

    run_composer_frames(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::Backspace, alt)]],
    );

    assert_eq!(input, "hello ");
    assert_eq!(cursor, 6);
}

#[test]
fn mac_cmd_backspace_deletes_to_line_start() {
    let mut input = "salut\nles amis".to_string();
    let mut cursor = input.chars().count();
    let cmd = egui::Modifiers {
        mac_cmd: true,
        command: true,
        ..egui::Modifiers::NONE
    };

    run_composer_frames(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::Backspace, cmd)]],
    );

    assert_eq!(input, "salut\n");
}

#[test]
fn ctrl_delete_removes_next_word() {
    let mut input = "hello world".to_string();
    let mut cursor = 0usize;
    let ctrl = egui::Modifiers {
        ctrl: true,
        command: true,
        ..egui::Modifiers::NONE
    };

    run_composer_frames(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::Delete, ctrl)]],
    );

    assert_eq!(input, " world");
    assert_eq!(cursor, 0);
}

#[test]
fn word_and_line_navigation_moves_cursor() {
    let alt = egui::Modifiers {
        alt: true,
        ..egui::Modifiers::NONE
    };
    let cmd = egui::Modifiers {
        mac_cmd: true,
        command: true,
        ..egui::Modifiers::NONE
    };

    let mut input = "hello world".to_string();
    let mut cursor = input.chars().count();
    run_composer_frames(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::ArrowLeft, alt)]],
    );
    assert_eq!(cursor, 6);

    run_composer_frames(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::ArrowLeft, cmd)]],
    );
    assert_eq!(cursor, 0);

    run_composer_frames(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::ArrowRight, cmd)]],
    );
    assert_eq!(cursor, input.chars().count());
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

// ── normalize_paste : retours à la ligne conservés ────────────

#[test]
fn paste_keeps_unix_newlines() {
    assert_eq!(
        normalize_paste("ligne un\nligne deux"),
        "ligne un\nligne deux"
    );
}

#[test]
fn paste_normalizes_windows_and_mac_line_endings() {
    assert_eq!(normalize_paste("a\r\nb\rc\nd"), "a\nb\nc\nd");
}

// ── cursor_from_point : placement ligne d'abord, puis colonne ──────────────

/// Positions de curseur pour « hello\nhi » : ligne 0 (y=0) longue jusqu'à
/// x=50, ligne 1 (y=22) courte jusqu'à x=16.
fn hello_hi_points() -> Vec<egui::Pos2> {
    vec![
        egui::pos2(0.0, 0.0),   // 0 : avant 'h'
        egui::pos2(10.0, 0.0),  // 1
        egui::pos2(20.0, 0.0),  // 2
        egui::pos2(30.0, 0.0),  // 3
        egui::pos2(40.0, 0.0),  // 4
        egui::pos2(50.0, 0.0),  // 5 : fin de « hello »
        egui::pos2(0.0, 22.0),  // 6 : début de « hi »
        egui::pos2(8.0, 22.0),  // 7
        egui::pos2(16.0, 22.0), // 8 : fin de « hi »
    ]
}

#[test]
fn click_right_of_short_line_stays_on_that_line() {
    // Clic loin à droite au niveau de la ligne courte (y=22) : le curseur doit
    // se poser en fin de « hi » (idx 8), PAS sauter sur la ligne du dessus
    // dont le texte s'étend plus loin — c'était le bug signalé.
    let points = hello_hi_points();
    let cursor = cursor_from_point(&points, egui::pos2(400.0, 22.0), 22.0);
    assert_eq!(cursor, 8);
}

#[test]
fn click_far_right_of_long_line_lands_at_its_end() {
    let points = hello_hi_points();
    let cursor = cursor_from_point(&points, egui::pos2(400.0, 0.0), 22.0);
    assert_eq!(cursor, 5);
}

#[test]
fn click_picks_nearest_column_on_target_line() {
    let points = hello_hi_points();
    // x=12 sur la ligne 0 → entre idx1 (10) et idx2 (20), plus proche de 10.
    assert_eq!(cursor_from_point(&points, egui::pos2(12.0, 0.0), 22.0), 1);
    // x=6 sur la ligne 1 → entre idx6 (0) et idx7 (8), plus proche de 8.
    assert_eq!(cursor_from_point(&points, egui::pos2(6.0, 22.0), 22.0), 7);
}

#[test]
fn click_below_all_text_clamps_to_last_line() {
    let points = hello_hi_points();
    // Clic bien en dessous (y=200) → dernière ligne, fin du texte.
    let cursor = cursor_from_point(&points, egui::pos2(400.0, 200.0), 22.0);
    assert_eq!(cursor, 8);
}

#[test]
fn cursor_from_point_empty_is_zero() {
    assert_eq!(
        cursor_from_point(&[egui::pos2(0.0, 0.0)], egui::pos2(9.0, 9.0), 22.0),
        0
    );
}

#[test]
fn caret_signature_distinguishes_every_input() {
    let base = super::caret_signature("bonjour", 18.0, 300.0, 2.0);
    assert_eq!(base, super::caret_signature("bonjour", 18.0, 300.0, 2.0));
    // Chaque paramètre change le tracé, donc la signature.
    assert_ne!(base, super::caret_signature("bonjou", 18.0, 300.0, 2.0));
    assert_ne!(base, super::caret_signature("bonjour", 20.0, 300.0, 2.0));
    assert_ne!(base, super::caret_signature("bonjour", 18.0, 301.0, 2.0));
    assert_ne!(base, super::caret_signature("bonjour", 18.0, 300.0, 1.0));
}

/// Régression : `menu_open_now` est vrai dès qu'un `:xyz` précède le curseur,
/// sans vérifier qu'une suggestion existe. Entrée partait donc sur
/// `AcceptShortcode`, qui échouait sans rien insérer : la touche ne faisait plus
/// rien du tout, alors qu'aucune popup n'était affichée.
#[test]
fn enter_without_matching_shortcode_falls_back_to_newline() {
    let mut input = "hello :zzz".to_string();
    let mut cursor = input.chars().count();

    let (submitted, _) = run_composer_session(
        &mut input,
        &mut cursor,
        vec![
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            vec![],
        ],
        true,
    );

    assert!(!submitted);
    assert_eq!(input, "hello :zzz\n");
    assert_eq!(cursor, input.chars().count());
}

#[test]
fn enter_with_matching_shortcode_still_accepts_it() {
    let mut input = "hello :jo".to_string();
    let mut cursor = input.chars().count();

    let (submitted, _) = run_composer_session(
        &mut input,
        &mut cursor,
        vec![
            vec![key_event(egui::Key::Enter, egui::Modifiers::NONE)],
            vec![],
        ],
        true,
    );

    assert!(!submitted);
    assert_eq!(input, "hello 😂");
}

/// Régression : le composeur est dimensionné avec le nombre de lignes d'AVANT
/// la frappe. Le défilement suiveur se basait sur cette hauteur périmée et
/// poussait le texte d'une ligne vers le haut dès qu'un retour à la ligne
/// apparaissait — alors que la frame suivante se contente d'agrandir le cadre.
#[test]
fn creating_a_line_does_not_scroll_the_composer() {
    let mut input = "hello".to_string();
    let mut cursor = input.chars().count();

    let (_, scroll) = run_composer_session(
        &mut input,
        &mut cursor,
        vec![vec![key_event(egui::Key::Enter, SHIFT)]],
        false,
    );

    assert_eq!(input, "hello\n");
    assert_eq!(scroll, 0.0);
}

#[test]
fn follow_caret_scroll_keeps_text_at_top_while_it_fits() {
    assert_eq!(follow_caret_scroll(0.0, 1.0, 2), 0.0);
    // Dernière ligne encore visible : toujours rien à faire défiler.
    assert_eq!(follow_caret_scroll(0.0, 9.0, MAX_VISIBLE_LINES), 0.0);
}

#[test]
fn follow_caret_scroll_follows_caret_beyond_the_visible_height() {
    // 12 lignes pour 10 visibles : le curseur en dernière ligne en défile 2.
    assert_eq!(follow_caret_scroll(0.0, 11.0, 12), 2.0);
    // Curseur au-dessus de la fenêtre : on recale sur sa ligne.
    assert_eq!(follow_caret_scroll(5.0, 1.0, 12), 1.0);
    // Curseur déjà visible : le défilement molette est conservé.
    assert_eq!(follow_caret_scroll(2.0, 5.0, 12), 2.0);
}
