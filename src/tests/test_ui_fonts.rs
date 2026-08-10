//! Couverture des polices : un caractère sans glyphe s'affiche en carré vide,
//! sans le moindre avertissement. Ce qui est écrit doit être dessinable.

use eframe::egui;

/// Caractères que l'interface écrit elle-même, et ceux qu'un texte collé
/// apporte couramment (documentation, notes, README).
const EXPECTED: &[char] = &[
    '☐', // case à cocher vide (listes de tâches Markdown)
    '✓', // case cochée, et coche des accusés dans les textes collés
    '→', '←', '↑', '↓', // flèches
    '⏎', // retour à la ligne
    '⌘', // raccourcis macOS
    '•', '–', '—', '…', // ponctuation typographique
    '«', '»', '“', '”', // guillemets
    '≥', '≤', '×', '÷', '±', // mathématiques courantes
    '€', '£', '©', '®', '°',
];

fn probe() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_fonts(super::build_fonts());
    // Les polices ne sont interrogeables qu'à l'intérieur d'une passe.
    ctx.begin_pass(egui::RawInput::default());
    ctx
}

#[test]
fn every_character_the_interface_writes_can_be_drawn() {
    let ctx = probe();
    let missing: Vec<char> = EXPECTED
        .iter()
        .copied()
        .filter(|c| {
            let text = c.to_string();
            !ctx.fonts_mut(|fonts| fonts.has_glyphs(&egui::FontId::proportional(14.0), &text))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "caractères sans glyphe, ils s'afficheront en carré vide : {missing:?}"
    );
}

#[test]
fn the_markdown_task_list_markers_can_be_drawn() {
    // Régression : `☑` n'était couvert par aucune police embarquée, si bien
    // qu'une tâche cochée s'affichait en carré vide.
    let ctx = probe();
    for marker in ["✓ ", "☐ "] {
        assert!(
            ctx.fonts_mut(|fonts| fonts.has_glyphs(&egui::FontId::proportional(14.0), marker)),
            "marqueur de liste de tâches non dessinable : {marker:?}"
        );
    }
}
