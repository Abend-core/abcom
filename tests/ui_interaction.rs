//! Tests d'interaction réels : kittest pilote l'interface via l'arbre
//! AccessKit, comme le ferait un lecteur d'écran ou un utilisateur.
//!
//! Le rendu headless de `src/tests/test_ui_app.rs` vérifie qu'on ne panique
//! pas ; ici on vérifie qu'un widget est bien là, porte le bon libellé et
//! réagit — ce qui est la seule chose qui rende une montée de version
//! d'egui vérifiable sans œil humain.

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

/// Écrire puis envoyer : la frappe atteint le composeur, et Cmd+Entrée déclenche
/// bien l'envoi plutôt que d'insérer une ligne.
#[test]
fn typing_then_command_enter_sends_the_message() {
    let submitted = RefCell::new(false);
    let text = RefCell::new(String::new());
    let mut harness = Harness::new_ui(|ui| {
        let mut buffer = text.borrow_mut();
        let response = ui.add(egui::TextEdit::singleline(&mut *buffer).hint_text("Message"));
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Message"));
        if ui.ctx().input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND,
                egui::Key::Enter,
            ))
        }) && !buffer.trim().is_empty()
        {
            *submitted.borrow_mut() = true;
            buffer.clear();
        }
    });

    harness.run();
    // Il faut d'abord donner le focus au champ, comme un utilisateur le ferait.
    harness.get_by_label("Message").click();
    harness.run();
    harness.get_by_label("Message").type_text("bonjour");
    harness.run();
    assert_eq!(
        *text.borrow(),
        "bonjour",
        "la frappe doit atteindre le champ"
    );

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    harness.run();
    assert!(*submitted.borrow(), "Cmd+Entrée doit envoyer");
    assert!(
        text.borrow().is_empty(),
        "le champ doit être vidé après l'envoi"
    );
}

/// Un résultat de recherche est cliquable et rapporte le message ciblé.
#[test]
fn clicking_a_search_result_selects_its_message() {
    let selected: RefCell<Option<u64>> = RefCell::new(None);
    let results = [(1_u64, "rendez-vous demain"), (2, "autre sujet")];
    let mut harness = Harness::new_ui(|ui| {
        for (hash, extrait) in results {
            let response = ui.add(egui::Label::new(extrait).sense(egui::Sense::click()));
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, extrait));
            if response.clicked() {
                *selected.borrow_mut() = Some(hash);
            }
        }
    });

    harness.run();
    harness.get_by_label("autre sujet").click();
    harness.run();

    assert_eq!(
        *selected.borrow(),
        Some(2),
        "le clic doit cibler le message de la ligne cliquée, pas la première"
    );
}

/// Échap ferme la surcouche la plus haute, et une seule à la fois.
#[test]
fn escape_closes_one_overlay_at_a_time() {
    let picker_open = RefCell::new(true);
    let settings_open = RefCell::new(true);
    let mut harness = Harness::new_ui(|ui| {
        if ui
            .ctx()
            .input_mut(|i| i.consume_shortcut(&abcom::ui::shortcuts::CLOSE_OVERLAY))
        {
            // Même ordre que `close_topmost_overlay` : le picker d'abord.
            if *picker_open.borrow() {
                *picker_open.borrow_mut() = false;
            } else {
                *settings_open.borrow_mut() = false;
            }
        }
        ui.label("fond");
    });

    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!*picker_open.borrow(), "le picker se ferme en premier");
    assert!(
        *settings_open.borrow(),
        "les paramètres ne doivent pas se fermer en même temps"
    );

    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(
        !*settings_open.borrow(),
        "le second Échap ferme les paramètres"
    );
}
