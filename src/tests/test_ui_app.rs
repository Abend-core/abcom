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
    let (_event_tx, event_rx) = mpsc::channel(16);
    let (_offer_tx, media_offer_rx) = mpsc::channel(16);
    AbcomApp::new(
        state,
        "aaaa:bbbb".to_string(),
        false,
        UiRuntimeChannels {
            event_rx,
            send_tx,
            send_media_tx,
            media_offer_rx,
            trust: Arc::new(TrustStore::new(HashMap::new(), None)),
        },
    )
}

/// Peint `frames` frames de l'arbre complet.
fn render(app: &mut AbcomApp, frames: usize) {
    let ctx = egui::Context::default();
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
