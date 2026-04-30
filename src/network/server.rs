use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;

use crate::message::{AppEvent, ChatMessage, GroupEvent, MessageAck, ReadReceipt, TypingIndicator};

use super::TCP_PORT;

/// Serveur TCP : écoute les connexions entrantes et dispatche les événements
pub async fn run_server(tx: Sender<AppEvent>) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", TCP_PORT)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[network] Erreur de bind TCP: {}", e);
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let tx = tx.clone();
                tokio::spawn(handle_incoming(stream, tx));
            }
            Err(_) => continue,
        }
    }
}

async fn handle_incoming(mut stream: TcpStream, tx: Sender<AppEvent>) {
    let mut buf = Vec::new();
    if stream.read_to_end(&mut buf).await.is_ok() && !buf.is_empty() {
        if let Ok(msg) = serde_json::from_slice::<ChatMessage>(&buf) {
            let _ = tx.send(AppEvent::MessageReceived(msg)).await;
        } else if let Ok(event) = serde_json::from_slice::<GroupEvent>(&buf) {
            let _ = tx.send(AppEvent::GroupEventReceived(event)).await;
        } else if let Ok(indicator) = serde_json::from_slice::<TypingIndicator>(&buf) {
            let _ = tx.send(AppEvent::UserTyping(indicator.from)).await;
        } else if let Ok(receipt) = serde_json::from_slice::<ReadReceipt>(&buf) {
            let _ = tx.send(AppEvent::ReadReceiptReceived(receipt)).await;
        } else if let Ok(ack) = serde_json::from_slice::<MessageAck>(&buf) {
            let _ = tx.send(AppEvent::MessageAckReceived(ack)).await;
        } else {
            eprintln!("[network] Message entrant non reconnu ({} bytes)", buf.len());
        }
    }
}
