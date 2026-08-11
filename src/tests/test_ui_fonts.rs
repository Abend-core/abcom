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
    /// Sonde la chaîne **privée de son dernier recours**.
    ///
    /// Unifont dessine une case hexadécimale pour n'importe quel codet, y
    /// compris non assigné : plus rien n'est jamais « absent », et la question
    /// utile devient « une *vraie* police le dessine-t-elle ? ». On la retire
    /// donc pour mesurer.
    fn without_last_resort() -> Self {
        let mut definitions = super::build_fonts();
        for family in definitions.families.values_mut() {
            family.retain(|name| name != "unifont");
        }
        let ctx = Self::context(definitions);
        let tofu = glyph_signature(&ctx, UNASSIGNED[0]);
        assert_eq!(
            tofu,
            glyph_signature(&ctx, UNASSIGNED[1]),
            "deux codets non assignés doivent produire le même carré vide, \
             sans quoi la référence de la mesure ne vaut rien"
        );
        Self { ctx, tofu }
    }

    /// Sonde la chaîne complète. La référence n'est plus le carré vide mais la
    /// case hexadécimale qu'Unifont réserve à un codet non assigné : un
    /// caractère qui s'en distingue a bien un glyphe à lui.
    fn complete() -> Self {
        let ctx = Self::context(super::build_fonts());
        let tofu = glyph_signature(&ctx, UNASSIGNED[0]);
        Self { ctx, tofu }
    }

    fn context(definitions: egui::FontDefinitions) -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_fonts(definitions);
        // Les polices ne sont interrogeables qu'à l'intérieur d'une passe.
        ctx.begin_pass(egui::RawInput::default());
        ctx
    }

    fn is_drawn(&self, c: char) -> bool {
        let signature = glyph_signature(&self.ctx, c);
        signature.is_some() && signature != self.tofu
    }
}

#[test]
fn every_character_the_interface_writes_is_drawn() {
    let probe = Probe::without_last_resort();
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

/// Un caractère témoin par police de la chaîne, qu'elle seule fournit : si
/// l'une d'elles se débranche, le témoin retombe en carré vide.
#[test]
fn each_font_of_the_chain_carries_its_share() {
    let probe = Probe::without_last_resort();
    for (character, font) in [
        ('✓', "Noto Sans"),
        ('✘', "Noto Sans Symbols 2"),
        ('→', "Inter"),
        ('😀', "polices d'emoji d'egui"),
    ] {
        assert!(
            probe.is_drawn(character),
            "{character} est peint en carré vide : {font} n'est plus consultée"
        );
    }

    // Le CJK n'existe que dans le dernier recours : sans lui, une phrase
    // entière en chinois, japonais ou coréen est illisible.
    let complete = Probe::complete();
    for character in ['好', '日', '한'] {
        assert!(
            complete.is_drawn(character),
            "{character} n'a pas de glyphe : Unifont n'est plus consultée"
        );
    }
}

/// La chaîne doit être consultée dans l'ordre déclaré, et Unifont fermer la
/// marche : placée plus haut, son rendu tramé remplacerait des glyphes que
/// d'autres polices dessinent proprement.
#[test]
fn the_chain_is_wired_in_declared_order() {
    let definitions = super::build_fonts();
    for name in super::FONT_CHAIN {
        assert!(
            definitions.font_data.contains_key(name),
            "{name} n'est pas embarquée"
        );
    }
    let proportional = &definitions.families[&egui::FontFamily::Proportional];
    assert_eq!(
        &proportional[..super::FONT_CHAIN.len()],
        &super::FONT_CHAIN.map(str::to_owned),
        "la chaîne doit être en tête du proportionnel, dans l'ordre"
    );
    assert_eq!(
        super::FONT_CHAIN.last(),
        Some(&"unifont"),
        "Unifont est le dernier recours, pas un choix par défaut"
    );
}
