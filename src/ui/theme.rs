//! Palette de l'interface, dérivée du thème actif.
//!
//! Les couleurs étaient auparavant écrites en dur à leur point d'usage, toutes
//! choisies pour un fond sombre : le sélecteur clair/sombre existait mais
//! n'était tenu qu'à moitié. Chaque rôle a désormais ses deux teintes ici, et
//! un seul endroit à modifier pour ajuster l'apparence.

use eframe::egui::{self, Color32};

/// Hauteur de ligne du composeur (positionnement du curseur et des emojis).
pub(crate) const LINE_HEIGHT: f32 = 22.0;

/// Couleurs de l'interface pour le thème courant.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    /// Fond des cartes, bulles et blocs de code.
    pub(crate) surface: Color32,
    /// Fond des champs de saisie et boutons d'action.
    pub(crate) surface_strong: Color32,
    /// Fond survolé d'un élément de liste.
    pub(crate) surface_hover: Color32,
    /// Liseré de séparation et bordure de cadre.
    pub(crate) separator: Color32,
    /// Texte principal.
    pub(crate) text: Color32,
    /// Texte secondaire discret (tailles, horodatages, libellés de repli).
    pub(crate) text_muted: Color32,
    /// Accent de l'application (sélection, boutons actifs).
    pub(crate) accent: Color32,
    /// Accent atténué (survol d'un bouton accentué).
    pub(crate) accent_soft: Color32,
    /// Erreur, échec de livraison, alerte.
    pub(crate) danger: Color32,
    /// Confirmation, pair en ligne.
    pub(crate) success: Color32,
    /// Accusé de lecture.
    pub(crate) receipt_read: Color32,
    /// Liens et citations de réponse.
    pub(crate) link: Color32,
    /// Fond des blocs de code, citations et en-têtes de tableau.
    pub(crate) block_bg: Color32,
    /// Fond alterné des lignes de tableau.
    pub(crate) block_stripe: Color32,
    /// Bordure des blocs et tableaux.
    pub(crate) block_border: Color32,
    /// Texte à l'intérieur d'un bloc de code.
    pub(crate) block_text: Color32,
}

/// Palette sombre — l'apparence historique de l'application.
pub(crate) const DARK: Palette = Palette {
    surface: Color32::from_rgb(43, 45, 49),
    surface_strong: Color32::from_rgb(66, 66, 70),
    surface_hover: Color32::from_rgb(60, 63, 68),
    separator: Color32::from_rgb(96, 96, 100),
    text: Color32::from_rgb(244, 245, 247),
    text_muted: Color32::from_gray(150),
    accent: Color32::from_rgb(88, 101, 242),
    accent_soft: Color32::from_rgb(88, 122, 255),
    danger: Color32::from_rgb(220, 80, 80),
    success: Color32::from_rgb(80, 200, 120),
    receipt_read: Color32::from_rgb(100, 180, 255),
    link: Color32::from_rgb(125, 211, 252),
    block_bg: Color32::from_rgb(30, 41, 59),
    block_stripe: Color32::from_rgb(23, 31, 46),
    block_border: Color32::from_rgb(71, 85, 105),
    block_text: Color32::from_rgb(248, 250, 252),
};

/// Palette claire — mêmes rôles, contrastes inversés.
pub(crate) const LIGHT: Palette = Palette {
    surface: Color32::from_rgb(241, 245, 249),
    surface_strong: Color32::from_rgb(226, 232, 240),
    surface_hover: Color32::from_rgb(214, 222, 233),
    separator: Color32::from_rgb(190, 198, 208),
    text: Color32::from_rgb(15, 23, 42),
    text_muted: Color32::from_rgb(71, 85, 105),
    accent: Color32::from_rgb(59, 74, 214),
    accent_soft: Color32::from_rgb(70, 100, 235),
    danger: Color32::from_rgb(180, 40, 40),
    success: Color32::from_rgb(21, 128, 61),
    receipt_read: Color32::from_rgb(3, 105, 161),
    link: Color32::from_rgb(3, 105, 161),
    block_bg: Color32::from_rgb(226, 232, 240),
    block_stripe: Color32::from_rgb(241, 245, 249),
    block_border: Color32::from_rgb(148, 163, 184),
    block_text: Color32::from_rgb(15, 23, 42),
};

/// Palette du thème actif.
pub(crate) fn palette(ui: &egui::Ui) -> Palette {
    for_dark_mode(ui.visuals().dark_mode)
}

/// Palette d'un mode donné, pour le code qui n'a qu'un `Visuals` sous la main.
pub(crate) fn for_dark_mode(dark_mode: bool) -> Palette {
    if dark_mode {
        DARK
    } else {
        LIGHT
    }
}

#[cfg(test)]
#[path = "../tests/test_ui_theme.rs"]
mod tests;
