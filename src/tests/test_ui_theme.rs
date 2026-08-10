use super::{for_dark_mode, DARK, LIGHT};

/// Un rôle doit rester lisible dans les deux thèmes : c'est précisément ce qui
/// manquait quand les couleurs étaient écrites en dur pour un fond sombre.
fn luminance(color: eframe::egui::Color32) -> f32 {
    let [r, g, b, _] = color.to_array();
    (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
}

#[test]
fn both_palettes_are_selectable() {
    assert_eq!(for_dark_mode(true).text, DARK.text);
    assert_eq!(for_dark_mode(false).text, LIGHT.text);
}

#[test]
fn text_contrasts_with_its_surface_in_both_themes() {
    for (name, palette) in [("sombre", DARK), ("clair", LIGHT)] {
        let text = luminance(palette.text);
        let surface = luminance(palette.surface);
        assert!(
            (text - surface).abs() > 0.4,
            "thème {name} : texte et fond trop proches ({text:.2} vs {surface:.2})"
        );
        let muted = luminance(palette.text_muted);
        assert!(
            (muted - surface).abs() > 0.15,
            "thème {name} : texte discret illisible ({muted:.2} vs {surface:.2})"
        );
    }
}

#[test]
fn dark_and_light_are_actually_inverted() {
    // Sans cela, une palette « claire » copiée de la sombre passerait inaperçue.
    assert!(
        luminance(DARK.surface) < 0.5,
        "le fond sombre doit être sombre"
    );
    assert!(
        luminance(LIGHT.surface) > 0.5,
        "le fond clair doit être clair"
    );
    assert!(luminance(DARK.text) > 0.5);
    assert!(luminance(LIGHT.text) < 0.5);
}
