use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::message::{AppEvent, ChatMessage, GroupEvent, SendRequest, SendGroupRequest, TypingRequest, TypingIndicator, ReadReceipt, ReadReceiptRequest, MessageAck, MessageAckRequest};

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
        // Try to parse as ChatMessage first
        if let Ok(msg) = serde_json::from_slice::<ChatMessage>(&buf) {
            let _ = tx.send(AppEvent::MessageReceived(msg)).await;
        }
        // If that fails, try GroupEvent
        else if let Ok(event) = serde_json::from_slice::<GroupEvent>(&buf) {
            let _ = tx.send(AppEvent::GroupEventReceived(event)).await;
        }
        // If that fails, try TypingIndicator
        else if let Ok(indicator) = serde_json::from_slice::<TypingIndicator>(&buf) {
            let _ = tx.send(AppEvent::UserTyping(indicator.from.clone())).await;
        }
        // If that fails, try ReadReceipt
        else if let Ok(receipt) = serde_json::from_slice::<ReadReceipt>(&buf) {
            let _ = tx.send(AppEvent::ReadReceiptReceived(receipt)).await;
        }
        // If that fails, try MessageAck
        else if let Ok(ack) = serde_json::from_slice::<MessageAck>(&buf) {
            let _ = tx.send(AppEvent::MessageAckReceived(ack)).await;
        }
        // If all fail, log error but don't crash
        else {
            eprintln!("[network] Failed to parse incoming message");
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

/// Expéditeur TCP pour les indicateurs de frappe (fire-and-forget, pas grave si ça rate)
pub async fn run_sender_typing(mut rx: Receiver<SendTypingRequest>) {
    while let Some(req) = rx.recv().await {
        tokio::spawn(async move {
            if let Ok(mut stream) = TcpStream::connect(req.to_addr).await {
                let typing = TypingIndicator { from: req.from };
                if let Ok(data) = serde_json::to_vec(&typing) {
                    let _ = stream.write_all(&data).await;
                    let _ = stream.flush().await;
                    let _ = stream.shutdown().await;
                }
            }
        });
    }
}

/// Expéditeur TCP pour les événements de groupe
pub async fn run_sender_group(mut rx: Receiver<SendGroupRequest>) {
    while let Some(req) = rx.recv().await {
        tokio::spawn(async move {
            match TcpStream::connect(req.to_addr).await {
                Ok(mut stream) => {
                    if let Ok(data) = serde_json::to_vec(&req.event) {
                        let _ = stream.write_all(&data).await;
                        let _ = stream.flush().await;
                        let _ = stream.shutdown().await;
                    }
                }
                Err(e) => {
                    eprintln!("[network] Erreur envoi GroupEvent vers {}: {}", req.to_addr, e);
                }
            }
        });
    }
}

/// Expéditeur TCP pour les indicateurs de typage
pub async fn run_sender_typing(mut rx: Receiver<TypingRequest>) {
    while let Some(req) = rx.recv().await {
        tokio::spawn(async move {
            if let Ok(mut stream) = TcpStream::connect(req.to_addr).await {
                if let Ok(data) = serde_json::to_vec(&req.indicator) {
                    let _ = stream.write_all(&data).await;
                    let _ = stream.flush().await;
                    let _ = stream.shutdown().await;
                }
            }
        });
    }
}

pub async fn run_sender_read_receipts(mut rx: Receiver<ReadReceiptRequest>) {
    while let Some(req) = rx.recv().await {
        tokio::spawn(async move {
            if let Ok(mut stream) = TcpStream::connect(req.to_addr).await {
                if let Ok(data) = serde_json::to_vec(&req.receipt) {
                    let _ = stream.write_all(&data).await;
                    let _ = stream.flush().await;
                    let _ = stream.shutdown().await;
                }
            }
        });
    }
}

pub async fn run_sender_ack(mut rx: Receiver<MessageAckRequest>) {
    while let Some(req) = rx.recv().await {
        tokio::spawn(async move {
            if let Ok(mut stream) = TcpStream::connect(req.to_addr).await {
                if let Ok(data) = serde_json::to_vec(&req.ack) {
                    let _ = stream.write_all(&data).await;
                    let _ = stream.flush().await;
                    let _ = stream.shutdown().await;
                }
            }
        });
    }
}
