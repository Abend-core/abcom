use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;

use crate::message::{MessageAckRequest, ReadReceiptRequest, SendGroupRequest, SendRequest, TypingRequest};

/// Expéditeur TCP pour les messages de chat
pub async fn run_sender(mut rx: Receiver<SendRequest>) {
    while let Some(req) = rx.recv().await {
        tokio::spawn(async move {
            match TcpStream::connect(req.to_addr).await {
                Ok(mut stream) => {
                    if let Ok(data) = serde_json::to_vec(&req.message) {
                        let _ = stream.write_all(&data).await;
                        let _ = stream.flush().await;
                        let _ = stream.shutdown().await;
                    }
                }
                Err(e) => eprintln!("[network] Connexion échouée vers {}: {}", req.to_addr, e),
            }
        });
    }
}

/// Expéditeur TCP pour les événements de groupe
pub async fn run_sender_group(mut rx: Receiver<SendGroupRequest>) {
    while let Some(req) = rx.recv().await {
        tokio::spawn(async move {
            if let Ok(mut stream) = TcpStream::connect(req.to_addr).await {
                if let Ok(data) = serde_json::to_vec(&req.event) {
                    let _ = stream.write_all(&data).await;
                    let _ = stream.flush().await;
                    let _ = stream.shutdown().await;
                }
            }
        });
    }
}

/// Expéditeur TCP pour les indicateurs de frappe (fire-and-forget)
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

/// Expéditeur TCP pour les accusés de lecture
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

/// Expéditeur TCP pour les ACK de livraison
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
