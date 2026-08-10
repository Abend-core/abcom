use super::ALL;
use crate::ui::UiLanguage;

/// Une entrée vide passerait inaperçue jusqu'à l'affichage.
#[test]
fn no_entry_is_empty() {
    for (name, entry) in ALL {
        assert!(!entry.fr.trim().is_empty(), "{name} : français vide");
        assert!(!entry.en.trim().is_empty(), "{name} : anglais vide");
    }
}

#[test]
fn every_entry_answers_in_both_languages() {
    for (name, entry) in ALL {
        assert_eq!(entry.get(UiLanguage::French), entry.fr, "{name}");
        assert_eq!(entry.get(UiLanguage::English), entry.en, "{name}");
    }
}

/// Deux clés au libellé strictement identique dans les deux langues sont un
/// doublon : elles devraient partager la même entrée. Un même français avec
/// deux anglais différents, en revanche, est légitime — « Emojis » titre de
/// fenêtre et « Emoji » infobulle de bouton.
#[test]
fn no_duplicate_entry() {
    let mut seen: std::collections::HashMap<(&str, &str), &str> = std::collections::HashMap::new();
    for (name, entry) in ALL {
        if let Some(previous) = seen.insert((entry.fr, entry.en), name) {
            panic!("« {} » défini deux fois : {previous} et {name}", entry.fr);
        }
    }
}

/// Le catalogue est le seul endroit où vivent les libellés : si le compte
/// s'effondre, c'est que des chaînes sont reparties se cacher dans l'UI.
#[test]
fn catalog_covers_the_whole_interface() {
    assert!(
        ALL.len() > 150,
        "catalogue anormalement réduit : {} entrées",
        ALL.len()
    );
}
