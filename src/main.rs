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
mod ui;

fn main() -> anyhow::Result<()> {
    let username = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "anonymous".to_string())
    });

    let state = Arc::new(Mutex::new(app::AppState::new(username.clone())));

    let (event_tx, event_rx) = mpsc::channel::<message::AppEvent>(256);
    let (send_tx, send_rx) = mpsc::channel::<message::SendRequest>(256);
    let (send_group_tx, send_group_rx) = mpsc::channel::<message::SendGroupRequest>(256);
    let (send_typing_tx, send_typing_rx) = mpsc::channel::<message::TypingRequest>(256);
    let (send_read_receipt_tx, send_read_receipt_rx) =
        mpsc::channel::<message::ReadReceiptRequest>(256);
    let (send_ack_tx, send_ack_rx) = mpsc::channel::<message::MessageAckRequest>(256);
    let (send_avatar_tx, send_avatar_rx) = mpsc::channel::<message::AvatarRequest>(64);
    let (send_media_tx, send_media_rx) = mpsc::channel::<message::MediaSendJob>(64);
    let (media_offer_tx, media_offer_rx) = mpsc::channel::<message::MediaStreamOffer>(16);

    let media_dir = config::data_dir().join("media");

    // Runtime tokio multi-thread — tourne en arrière-plan pendant qu'egui
    // occupe le thread principal.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.spawn(discovery::run(username.clone(), event_tx.clone()));
    rt.spawn(network::run_server(event_tx.clone()));
    rt.spawn(network::run_sender(send_rx));
    rt.spawn(network::run_sender_group(send_group_rx));
    rt.spawn(network::run_sender_typing(send_typing_rx));
    rt.spawn(network::run_sender_read_receipts(send_read_receipt_rx));
    rt.spawn(network::run_sender_ack(send_ack_rx));
    rt.spawn(network::run_sender_avatar(send_avatar_rx));
    rt.spawn(network::run_media_sender(send_media_rx, event_tx.clone()));
    rt.spawn(network::run_media_server(
        event_tx.clone(),
        media_offer_tx,
        media_dir,
    ));

    ui::run(
        state,
        event_rx,
        send_tx,
        send_group_tx,
        send_typing_tx,
        send_read_receipt_tx,
        send_ack_tx,
        send_avatar_tx,
        send_media_tx,
        media_offer_rx,
    )?;

    Ok(())
}
