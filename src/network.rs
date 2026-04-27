use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::message::{AppEvent, ChatMessage, SendRequest};

pub const TCP_PORT: u16 = 9000;

/// Serveur TCP : écoute les messages entrants et les transmet à l'UI
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
        }
    }
}

/// Expéditeur TCP : reçoit les demandes d'envoi et les achemine
pub async fn run_sender(mut rx: Receiver<SendRequest>) {
    while let Some(req) = rx.recv().await {
        tokio::spawn(async move {
            match TcpStream::connect(req.to_addr).await {
                Ok(mut stream) => {
                    if let Ok(data) = serde_json::to_vec(&req.message) {
                        let _ = stream.write_all(&data).await;
                        let _ = stream.flush().await;
                        // Fermer l'écriture pour que read_to_end côté serveur termine
                        let _ = stream.shutdown().await;
                    }
                }
                Err(e) => {
                    eprintln!("[network] Connexion échouée vers {}: {}", req.to_addr, e);
                }
            }
        });
    }
}
