//! Constantes visuelles partagées par plusieurs écrans, pour éviter que la
//! même valeur (hauteur de ligne, couleur) soit dupliquée en dur à plusieurs
//! endroits et dérive silencieusement au fil des modifications.
//!
//! Ne recense que les valeurs réellement dupliquées ailleurs dans l'UI — les
//! teintes proches mais choisies indépendamment (gris 140/160/165/190…)
//! restent en dur à leur point d'usage : les regrouper créerait un couplage
//! visuel qui n'existe pas aujourd'hui.

use eframe::egui;

/// Hauteur de ligne du composeur (positionnement du curseur et des emojis).
pub(crate) const LINE_HEIGHT: f32 = 22.0;

/// Liseré de séparation (barre d'actions du composeur).
pub(crate) const SEPARATOR: egui::Color32 = egui::Color32::from_rgb(96, 96, 100);

/// Texte secondaire discret (tailles de fichiers, libellés de repli, compteur
/// de saisie sous le seuil d'alerte).
pub(crate) const TEXT_MUTED: egui::Color32 = egui::Color32::from_gray(150);
