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

// ─── Placement de la popup d'emojis ─────────────────────────────────────────

use super::{popup_pos, PICKER_SIZE};
use eframe::egui;

/// Fenêtre de 1000 × 700, comme une fenêtre applicative ordinaire.
fn screen() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 700.0))
}

fn button_at(x: f32, y: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(28.0, 28.0))
}

#[test]
fn popup_opens_below_the_button_when_there_is_room() {
    let pos = popup_pos(screen(), button_at(120.0, 40.0), PICKER_SIZE);
    assert_eq!(pos.x, 120.0);
    // 40 + 28 (bouton) + 6 (écart).
    assert_eq!(pos.y, 74.0);
}

/// Régression : le picker des réactions s'ouvrait toujours sous le « + ». Sur
/// un message du bas de la conversation — le cas courant — ses 340 px
/// sortaient de la fenêtre et la grille arrivait tronquée.
#[test]
fn popup_flips_above_the_button_when_it_would_overflow() {
    let pos = popup_pos(screen(), button_at(120.0, 620.0), PICKER_SIZE);
    // 620 - 6 (écart) - 340 (hauteur).
    assert_eq!(pos.y, 274.0);
    assert!(pos.y >= 8.0);
}

#[test]
fn popup_stays_inside_the_window_horizontally() {
    // Bouton collé au bord droit (barre de saisie) : la popup rentre.
    let pos = popup_pos(screen(), button_at(960.0, 620.0), PICKER_SIZE);
    assert_eq!(pos.x, 1000.0 - 8.0 - PICKER_SIZE.x);

    // Bouton à cheval sur le bord gauche : la marge est respectée.
    let pos = popup_pos(screen(), button_at(-20.0, 40.0), PICKER_SIZE);
    assert_eq!(pos.x, 8.0);
}

/// Fenêtre plus petite que la popup : aucun placement n'est satisfaisant, mais
/// le coin haut-gauche doit rester visible plutôt que de partir hors écran.
#[test]
fn popup_prefers_the_top_left_corner_on_a_tiny_window() {
    let tiny = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
    let pos = popup_pos(tiny, button_at(60.0, 150.0), PICKER_SIZE);
    assert_eq!(pos, egui::pos2(8.0, 8.0));
}
