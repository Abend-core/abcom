//! Tests d'interaction réels : kittest pilote l'interface via l'arbre
//! AccessKit, comme le ferait un lecteur d'écran ou un utilisateur.
//!
//! C'est ce que le rendu headless de `src/tests/test_ui_app.rs` ne sait pas
//! faire : celui-ci vérifie qu'on ne panique pas, celui-là qu'un widget est
//! bien là, porte le bon libellé et réagit au clic.

use std::cell::RefCell;

use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;

/// Reproduit le contrat de nos boutons peints : sans `widget_info`, ils sont
/// introuvables dans l'arbre d'accessibilité — donc invisibles pour un lecteur
/// d'écran comme pour un test piloté.
#[test]
fn painted_buttons_expose_an_accessible_label() {
    let clicked = RefCell::new(false);
    let mut harness = Harness::new_ui(|ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
        ui.painter()
            .rect_filled(rect, 2.0, egui::Color32::from_gray(80));
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Retirer la pièce jointe")
        });
        if response.clicked() {
            *clicked.borrow_mut() = true;
        }
    });

    harness.run();
    harness.get_by_label("Retirer la pièce jointe").click();
    harness.run();
    assert!(
        *clicked.borrow(),
        "le bouton peint doit être atteignable et cliquable via AccessKit"
    );
}

/// Le raccourci de recherche doit être consommé par l'application avant
/// qu'un champ de saisie ne le voie passer.
#[test]
fn search_shortcut_is_consumed_before_widgets_see_it() {
    let opened = RefCell::new(false);
    let typed = RefCell::new(String::new());
    let mut harness = Harness::new_ui(|ui| {
        if ui
            .ctx()
            .input_mut(|i| i.consume_shortcut(&abcom::ui::shortcuts::SEARCH))
        {
            *opened.borrow_mut() = true;
        }
        let mut buffer = typed.borrow_mut();
        ui.add(egui::TextEdit::singleline(&mut *buffer).hint_text("saisie"));
    });

    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run();

    assert!(*opened.borrow(), "Cmd+F doit ouvrir la recherche");
    assert!(
        typed.borrow().is_empty(),
        "le raccourci ne doit pas atteindre le champ de saisie"
    );
}
