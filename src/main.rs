use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

mod app;
mod archive;
mod config;
mod discovery;
mod emoji_registry;
mod klipy;
mod message;
mod network;
mod notify;
mod ui;

fn main() -> anyhow::Result<()> {
    if let Ok(content) = std::fs::read_to_string(".env") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                std::env::set_var(k.trim(), v.trim());
            }
        }
    }

    let username = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "anonymous".to_string())
    });

    let state = Arc::new(Mutex::new(app::AppState::new(username.clone())));

    // Contexte egui partagé avec les tâches de fond : renseigné au lancement
    // de l'UI, il permet de la réveiller à chaque événement (cf. notify.rs).
    let ui_ctx: notify::UiContext = Arc::new(std::sync::OnceLock::new());

    let (send_tx, send_rx) = mpsc::channel::<message::SendRequest>(256);
    let (send_group_tx, send_group_rx) = mpsc::channel::<message::SendGroupRequest>(256);
    let (send_typing_tx, send_typing_rx) = mpsc::channel::<message::TypingRequest>(256);
    let (send_read_receipt_tx, send_read_receipt_rx) =
        mpsc::channel::<message::ReadReceiptRequest>(256);
    let (send_ack_tx, send_ack_rx) = mpsc::channel::<message::MessageAckRequest>(256);
    let (send_avatar_tx, send_avatar_rx) = mpsc::channel::<message::AvatarRequest>(64);
    let (send_reaction_tx, send_reaction_rx) = mpsc::channel::<message::ReactionRequest>(256);
    let (send_media_tx, send_media_rx) = mpsc::channel::<message::MediaSendJob>(64);

    let media_dir = config::data_dir().join("media");

    // GC du cache disque des médias (orphelins + plafond), hors chemin de
    // démarrage : l'UI s'ouvre sans attendre le parcours du dossier.
    {
        let referenced: std::collections::HashSet<String> = state
            .lock()
            .unwrap()
            .messages
            .iter()
            .filter_map(|m| m.media.as_ref().map(|x| x.id.clone()))
            .collect();
        let dir = media_dir.clone();
        std::thread::spawn(move || app::media::gc_media_dir(dir, referenced));
    }

    // Runtime tokio multi-thread — tourne en arrière-plan pendant qu'egui
    // occupe le thread principal. Deux workers suffisent largement : les
    // tâches sont du réseau/disque bufferisé, pas du calcul.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    // Canaux vers l'UI : chaque événement relayé réveille egui.
    let (event_tx, event_rx) =
        notify::ui_channel::<message::AppEvent>(256, ui_ctx.clone(), &rt);
    let (media_offer_tx, media_offer_rx) =
        notify::ui_channel::<message::MediaStreamOffer>(16, ui_ctx.clone(), &rt);

    rt.spawn(discovery::run(username.clone(), event_tx.clone()));
    rt.spawn(network::run_server(event_tx.clone()));
    rt.spawn(network::run_sender(send_rx));
    rt.spawn(network::run_sender_group(send_group_rx));
    rt.spawn(network::run_sender_typing(send_typing_rx));
    rt.spawn(network::run_sender_read_receipts(send_read_receipt_rx));
    rt.spawn(network::run_sender_ack(send_ack_rx));
    rt.spawn(network::run_sender_avatar(send_avatar_rx));
    rt.spawn(network::run_sender_reaction(send_reaction_rx));
    rt.spawn(network::run_media_sender(send_media_rx, event_tx.clone()));
    rt.spawn(network::run_media_server(
        event_tx.clone(),
        media_offer_tx,
        media_dir,
    ));

    ui::run(
        state,
        ui_ctx,
        event_rx,
        send_tx,
        send_group_tx,
        send_typing_tx,
        send_read_receipt_tx,
        send_ack_tx,
        send_avatar_tx,
        send_reaction_tx,
        send_media_tx,
        media_offer_rx,
    )?;

    Ok(())
}
