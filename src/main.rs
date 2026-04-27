use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

mod app;
mod discovery;
mod emoji_registry;
mod message;
mod network;
mod ui;

fn main() -> anyhow::Result<()> {
    let username = std::env::args()
        .nth(1)
        .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "anonymous".to_string()));

    let state = Arc::new(Mutex::new(app::AppState::new(username.clone())));

    let (event_tx, event_rx) = mpsc::channel::<message::AppEvent>(256);
    let (send_tx, send_rx) = mpsc::channel::<message::SendRequest>(256);

    // Runtime tokio multi-thread — tourne en arrière-plan pendant qu'egui
    // occupe le thread principal.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.spawn(discovery::run(username.clone(), event_tx.clone()));
    rt.spawn(network::run_server(event_tx.clone()));
    rt.spawn(network::run_sender(send_rx));

    ui::run(state, event_rx, send_tx)?;

    Ok(())
}
