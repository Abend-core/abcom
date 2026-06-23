use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;

use crate::config;
use crate::message::{AppEvent, NetworkPacket};

/// Serveur TCP : écoute les connexions entrantes et dispatche les événements
pub async fn run_server(tx: Sender<AppEvent>) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", config::chat_port())).await {
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
        match serde_json::from_slice::<NetworkPacket>(&buf) {
            Ok(NetworkPacket::Chat(msg))          => { let _ = tx.send(AppEvent::MessageReceived(msg)).await; }
            Ok(NetworkPacket::Group(event))       => { let _ = tx.send(AppEvent::GroupEventReceived(event)).await; }
            Ok(NetworkPacket::Typing(indicator))  => { let _ = tx.send(AppEvent::UserTyping(indicator.from)).await; }
            Ok(NetworkPacket::ReadReceipt(r))     => { let _ = tx.send(AppEvent::ReadReceiptReceived(r)).await; }
            Ok(NetworkPacket::Ack(ack))           => { let _ = tx.send(AppEvent::MessageAckReceived(ack)).await; }
            Err(_) => eprintln!("[network] Paquet entrant non reconnu ({} bytes)", buf.len()),
        }
    }
}
