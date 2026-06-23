use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver;

use crate::message::{MessageAckRequest, NetworkPacket, ReadReceiptRequest, SendGroupRequest, SendRequest, TypingRequest};

async fn send_packet(addr: std::net::SocketAddr, packet: NetworkPacket) {
    match TcpStream::connect(addr).await {
        Ok(mut stream) => {
            if let Ok(data) = serde_json::to_vec(&packet) {
                let _ = stream.write_all(&data).await;
                let _ = stream.flush().await;
                let _ = stream.shutdown().await;
            }
        }
        Err(e) => eprintln!("[network] Connexion échouée vers {}: {}", addr, e),
    }
}

/// Expéditeur TCP pour les messages de chat
pub async fn run_sender(mut rx: Receiver<SendRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Chat(req.message);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les événements de groupe
pub async fn run_sender_group(mut rx: Receiver<SendGroupRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Group(req.event);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les indicateurs de frappe (fire-and-forget)
pub async fn run_sender_typing(mut rx: Receiver<TypingRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Typing(req.indicator);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les accusés de lecture
pub async fn run_sender_read_receipts(mut rx: Receiver<ReadReceiptRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::ReadReceipt(req.receipt);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}

/// Expéditeur TCP pour les ACK de livraison
pub async fn run_sender_ack(mut rx: Receiver<MessageAckRequest>) {
    while let Some(req) = rx.recv().await {
        let packet = NetworkPacket::Ack(req.ack);
        tokio::spawn(send_packet(req.to_addr, packet));
    }
}
