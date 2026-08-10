//! Couverture des polices : un caractère sans glyphe s'affiche en carré vide,
//! sans le moindre avertissement. Ce qui est écrit doit être dessinable.
//!
//! La mesure passe par le **glyphe réellement peint**, pas par
//! `Fonts::has_glyphs` : celui-ci répond « absent » pour tout caractère porté
//! par la police de remplacement de la famille, ce qui en fait un faux négatif
//! sur une bonne part des symboles.

use eframe::egui;

/// Caractères que l'interface écrit elle-même, et ceux qu'un texte collé
/// apporte couramment (documentation, notes, README).
const EXPECTED: &[char] = &[
    '☐', '☑', // cases à cocher des listes de tâches Markdown
    '✓', '✔', '✘', // coches et croix des accusés et des tableaux
    '→', '←', '↑', '↓', // flèches
    '⏎', '␣', // touches
    '⌘', // raccourcis macOS
    '•', '–', '—', '…', // ponctuation typographique
    '«', '»', '“', '”', // guillemets
    '≥', '≤', '×', '÷', '±', // mathématiques courantes
    '€', '£', '©', '®', '°',
];

/// Codets non assignés par Unicode : aucune police ne peut les dessiner, ils
/// donnent donc la signature du carré vide.
const UNASSIGNED: [char; 2] = ['\u{0378}', '\u{05FF}'];

/// Signature du glyphe peint pour `c` : sa position dans l'atlas et sa chasse.
/// Deux caractères qui la partagent sont peints à l'identique.
fn glyph_signature(ctx: &egui::Context, c: char) -> Option<(u16, u16, u16, u16, u32)> {
    let galley = ctx.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            c.to_string(),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        )
    });
    let glyph = galley.rows.first()?.glyphs.first()?;
    let uv = glyph.uv_rect;
    Some((
        uv.min[0],
        uv.min[1],
        uv.max[0],
        uv.max[1],
        glyph.advance_width.to_bits(),
    ))
}

struct Probe {
    ctx: egui::Context,
    tofu: Option<(u16, u16, u16, u16, u32)>,
}

impl Probe {
    fn new() -> Self {
        let ctx = egui::Context::default();
        ctx.set_fonts(super::build_fonts());
        // Les polices ne sont interrogeables qu'à l'intérieur d'une passe.
        ctx.begin_pass(egui::RawInput::default());
        let tofu = glyph_signature(&ctx, UNASSIGNED[0]);
        assert_eq!(
            tofu,
            glyph_signature(&ctx, UNASSIGNED[1]),
            "deux codets non assignés doivent produire le même carré vide, \
             sans quoi la référence de la mesure ne vaut rien"
        );
        Self { ctx, tofu }
    }

    fn is_drawn(&self, c: char) -> bool {
        let signature = glyph_signature(&self.ctx, c);
        signature.is_some() && signature != self.tofu
    }
}

#[test]
fn every_character_the_interface_writes_is_drawn() {
    let probe = Probe::new();
    let missing: Vec<char> = EXPECTED
        .iter()
        .copied()
        .filter(|c| !probe.is_drawn(*c))
        .collect();
    assert!(
        missing.is_empty(),
        "caractères peints en carré vide : {missing:?}"
    );
}

/// Les deux polices de repli doivent rester branchées sur les familles
/// standard : sans elles, une part entière des symboles retombe en carré vide.
/// Un caractère témoin par police, qu'elle seule fournit.
#[test]
fn both_fallback_fonts_are_wired_in() {
    let probe = Probe::new();
    for (character, font) in [('✓', "Inter"), ('✘', "Noto Sans Symbols 2")] {
        assert!(
            probe.is_drawn(character),
            "{character} est peint en carré vide : {font} n'est plus consultée en repli"
        );
    }
}
