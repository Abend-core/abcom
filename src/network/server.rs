use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;
use tokio::time::{timeout, Duration};

use crate::config;
use crate::message::{AppEvent, NetworkPacket};

// 64 KB — bien au-delà du plus long message texte légitime (~16 000 mots)
const MAX_PACKET_SIZE: u64 = 64 * 1024;
const READ_TIMEOUT_SECS: u64 = 5;

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

async fn handle_incoming(stream: TcpStream, tx: Sender<AppEvent>) {
    let mut buf = Vec::new();

    let result = timeout(
        Duration::from_secs(READ_TIMEOUT_SECS),
        stream.take(MAX_PACKET_SIZE + 1).read_to_end(&mut buf),
    )
    .await;

    match result {
        Err(_) => {
            eprintln!("[network] Timeout sur la connexion entrante");
            return;
        }
        Ok(Err(e)) => {
            eprintln!("[network] Erreur de lecture: {}", e);
            return;
        }
        Ok(Ok(_)) => {}
    }

    if buf.is_empty() {
        return;
    }

    if buf.len() > MAX_PACKET_SIZE as usize {
        eprintln!(
            "[network] Paquet trop volumineux ({} bytes), ignoré",
            buf.len()
        );
        return;
    }

    match serde_json::from_slice::<NetworkPacket>(&buf) {
        Ok(NetworkPacket::Chat(msg)) => {
            let _ = tx.send(AppEvent::MessageReceived(msg)).await;
        }
        Ok(NetworkPacket::Group(event)) => {
            let _ = tx.send(AppEvent::GroupEventReceived(event)).await;
        }
        Ok(NetworkPacket::Typing(indicator)) => {
            let _ = tx.send(AppEvent::UserTyping(indicator.from)).await;
        }
        Ok(NetworkPacket::ReadReceipt(r)) => {
            let _ = tx.send(AppEvent::ReadReceiptReceived(r)).await;
        }
        Ok(NetworkPacket::Ack(ack)) => {
            let _ = tx.send(AppEvent::MessageAckReceived(ack)).await;
        }
        Ok(NetworkPacket::Avatar(announce)) => {
            let _ = tx.send(AppEvent::AvatarReceived(announce)).await;
        }
        Ok(NetworkPacket::Reaction(event)) => {
            let _ = tx.send(AppEvent::ReactionReceived(event)).await;
        }
        Err(_) => eprintln!("[network] Paquet entrant non reconnu ({} bytes)", buf.len()),
    }
}

#[cfg(test)]
#[path = "../tests/test_network_server.rs"]
mod tests;
