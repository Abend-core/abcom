//! Rendu headless de l'arbre d'interface complet : détecte panique, identifiants
//! egui dupliqués et ordre de panneaux invalide sans ouvrir de fenêtre.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use eframe::egui;
use tokio::sync::mpsc;

use super::{AbcomApp, UiRuntimeChannels};
use crate::app::AppState;
use crate::network::secure::TrustStore;

fn test_app() -> AbcomApp {
    let dir = std::env::temp_dir().join(format!(
        "abcom-ui-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let state = Arc::new(Mutex::new(AppState::new_with_base("moi", &dir)));
    let (send_tx, _send_rx) = mpsc::channel(16);
    let (send_media_tx, _media_rx) = mpsc::channel(16);
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_offer_tx, media_offer_rx) = mpsc::channel(16);
    AbcomApp::new(
        state,
        "aaaa:bbbb".to_string(),
        false,
        UiRuntimeChannels {
            event_rx,
            event_tx,
            send_tx,
            send_media_tx,
            media_offer_rx,
            trust: Arc::new(TrustStore::new(HashMap::new(), None)),
        },
    )
}

/// Peint `frames` frames de l'arbre complet, dans les deux thèmes.
fn render(app: &mut AbcomApp, frames: usize) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        render_with_theme(app, frames, theme);
    }
}

fn render_with_theme(app: &mut AbcomApp, frames: usize, theme: egui::Theme) {
    let ctx = egui::Context::default();
    ctx.set_theme(theme);
    for _ in 0..frames {
        let mut output = ctx.run_ui(egui::RawInput::default(), |root| {
            app.show_sidebar_panel(root);
            let (emoji, gif) = app.show_input_bar(root);
            let ctx = root.ctx().clone();
            app.show_notification(&ctx);
            app.show_emoji_picker_window(&ctx, emoji);
            app.show_gif_picker_window(&ctx, gif);
            app.render_group_modal(&ctx);
            app.render_group_manage_modal(&ctx);
            app.show_central_panel(root);
            app.show_reaction_emoji_picker(&ctx);
            app.render_settings(&ctx);
            app.show_media_viewer(&ctx);
            app.show_search(&ctx);
        });
        // epaint panique si les deltas de textures d'une frame sont abandonnés.
        output.textures_delta.clear();
    }
}

#[test]
fn renders_the_whole_tree_without_panicking() {
    let mut app = test_app();
    render(&mut app, 3);
}

#[test]
fn renders_with_messages_and_a_selected_conversation() {
    let mut app = test_app();
    {
        let mut s = app.state.lock().unwrap();
        s.add_peer("alice".into(), "127.0.0.1:9000".parse().unwrap());
        s.add_message(crate::message::ChatMessage {
            from: "alice".into(),
            content: "**gras** et `code` :tada:".into(),
            timestamp: "12:00".into(),
            timestamp_epoch: Some(1),
            to_user: Some("moi".into()),
            media: None,
            reply_to: None,
            nonce: None,
        });
        s.selected_conversation = Some("alice".into());
    }
    render(&mut app, 3);
}

#[test]
fn survives_a_hide_and_restore_cycle() {
    let mut app = test_app();
    let ctx = egui::Context::default();
    render(&mut app, 1);
    // Libère textures et images du chargeur.
    app.hide_to_tray(&ctx);
    app.show_from_tray(&ctx);
    // Tout doit se reconstruire : un rendu qui panique ici signalerait qu'on a
    // libéré une ressource qu'egui croyait encore allouée.
    render(&mut app, 2);
}

#[test]
fn renders_every_modal_and_picker_open() {
    let mut app = test_app();
    app.modals.settings_open = true;
    app.modals.group_modal_open = true;
    app.modals.key_mismatch = Some(("alice".into(), vec![7u8; 32]));
    app.modals.rename_target = Some("alice".into());
    app.modals.participants_open = true;
    app.show_emoji_picker = true;
    app.gif_picker.show = true;
    app.last_notification = Some("coucou".into());
    app.search.open = true;
    app.search.query = "bureau".into();
    render(&mut app, 3);
}

/// Les onglets de Paramètres ne s'ouvraient dans aucun test : Crédits et
/// Licence n'étaient jamais peints, donc ni panique ni identifiant egui
/// dupliqué n'y auraient été détectés.
#[test]
fn renders_every_settings_tab() {
    use crate::ui::SettingsTab;

    let mut app = test_app();
    app.modals.settings_open = true;
    for tab in [
        SettingsTab::Profile,
        SettingsTab::General,
        SettingsTab::Storage,
        SettingsTab::Credits,
        SettingsTab::License,
    ] {
        app.modals.settings_tab = tab;
        render(&mut app, 2);
    }
}

/// `process_events` tient le verrou d'état pendant toute sa boucle : un
/// gestionnaire qui le reprend fige l'application entière, fenêtre comprise —
/// c'est arrivé sur le compte rendu de purge, l'utilisateur ne pouvait plus ni
/// fermer ni ouvrir quoi que ce soit.
///
/// `AbcomApp` n'est pas `Send` (le tray), donc pas de chien de garde sur un
/// autre thread : un interblocage se manifeste ici en ne rendant jamais la
/// main. Les assertions vérifient que l'événement a bien été traité.
#[test]
fn a_purge_report_is_handled_without_retaking_the_state_lock() {
    use crate::app::media::GcReport;
    use crate::message::AppEvent;

    let mut app = test_app();
    let report = |dry_run| {
        AppEvent::MediaPurged(GcReport {
            freed_bytes: 1024,
            freed_files: 2,
            dry_run,
        })
    };

    app.net.event_tx.try_send(report(true)).unwrap();
    app.process_events();
    assert_eq!(
        app.purge_preview.map(|r| r.freed_bytes),
        Some(1024),
        "une simulation doit alimenter l'aperçu"
    );
    assert!(!app.purge_preview_pending);

    app.net.event_tx.try_send(report(false)).unwrap();
    app.process_events();
    assert!(
        app.purge_preview.is_none(),
        "une purge réelle périme l'aperçu"
    );
    let notice = app.last_notification.expect("purge annoncée");
    assert!(notice.contains("1.0 ko"), "compte rendu chiffré : {notice}");
}

/// Le rattrapage d'accusés de lecture doit fonctionner sans changer de
/// conversation — c'est ce que `logic` déclenche au retour du focus — et ne
/// pas réémettre à chaque passage.
#[test]
fn read_receipts_are_swept_without_switching_conversation() {
    let mut app = test_app();
    let (send_tx, mut send_rx) = mpsc::channel(64);
    app.net.send_tx = send_tx;

    let conv = {
        let mut s = app.state.lock().unwrap();
        s.peers.push(crate::app::Peer {
            username: "alice".to_string(),
            addr: "127.0.0.1:9000".parse().unwrap(),
            last_seen: 0,
            online: true,
        });
        let group = s
            .create_group("equipe".to_string(), vec!["alice".to_string()])
            .expect("création du salon");
        let conv = AppState::group_conv_key(&group.id);
        // Message reçu pendant que la fenêtre n'avait pas le focus.
        s.add_message(crate::message::ChatMessage {
            from: "alice".to_string(),
            content: "coucou".to_string(),
            timestamp: "12:00".to_string(),
            timestamp_epoch: Some(1),
            to_user: Some(conv.clone()),
            media: None,
            reply_to: None,
            nonce: Some(1),
        });
        s.selected_conversation = Some(conv.clone());
        conv
    };

    app.send_read_receipts_for_conversation(Some(conv.clone()));
    assert!(
        send_rx.try_recv().is_ok(),
        "l'accusé doit partir sans quitter puis rouvrir le salon"
    );

    app.send_read_receipts_for_conversation(Some(conv));
    assert!(
        send_rx.try_recv().is_err(),
        "et ne pas être réémis au passage suivant"
    );
}
