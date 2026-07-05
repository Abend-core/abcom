//! Expéditeurs : les canaux typés de l'UI convergent vers le
//! [`ConnectionPool`](super::pool::ConnectionPool), qui maintient **une
//! connexion persistante et chiffrée par pair** (plus de connexion TCP par
//! paquet).

use std::sync::Arc;

use tokio::sync::mpsc::Receiver;

use crate::message::{
    AvatarRequest, MessageAckRequest, NetworkPacket, ReactionRequest, ReadReceiptRequest,
    SendGroupRequest, SendRequest, TypingRequest,
};

use super::pool::ConnectionPool;

/// Expéditeur pour les messages de chat.
pub async fn run_sender(mut rx: Receiver<SendRequest>, pool: Arc<ConnectionPool>) {
    while let Some(req) = rx.recv().await {
        pool.send(req.to_addr, NetworkPacket::Chat(req.message))
            .await;
    }
}

/// Expéditeur pour les événements de groupe.
pub async fn run_sender_group(mut rx: Receiver<SendGroupRequest>, pool: Arc<ConnectionPool>) {
    while let Some(req) = rx.recv().await {
        pool.send(req.to_addr, NetworkPacket::Group(req.event))
            .await;
    }
}

/// Expéditeur pour les indicateurs de frappe (fire-and-forget).
pub async fn run_sender_typing(mut rx: Receiver<TypingRequest>, pool: Arc<ConnectionPool>) {
    while let Some(req) = rx.recv().await {
        pool.send(req.to_addr, NetworkPacket::Typing(req.indicator))
            .await;
    }
}

/// Expéditeur pour les accusés de lecture.
pub async fn run_sender_read_receipts(
    mut rx: Receiver<ReadReceiptRequest>,
    pool: Arc<ConnectionPool>,
) {
    while let Some(req) = rx.recv().await {
        pool.send(req.to_addr, NetworkPacket::ReadReceipt(req.receipt))
            .await;
    }
}

/// Expéditeur pour les annonces d'avatar (image de profil).
pub async fn run_sender_avatar(mut rx: Receiver<AvatarRequest>, pool: Arc<ConnectionPool>) {
    while let Some(req) = rx.recv().await {
        pool.send(req.to_addr, NetworkPacket::Avatar(req.announce))
            .await;
    }
}

/// Expéditeur pour les ACK de livraison.
pub async fn run_sender_ack(mut rx: Receiver<MessageAckRequest>, pool: Arc<ConnectionPool>) {
    while let Some(req) = rx.recv().await {
        pool.send(req.to_addr, NetworkPacket::Ack(req.ack)).await;
    }
}

/// Expéditeur pour les réactions emoji (ajout/retrait).
pub async fn run_sender_reaction(mut rx: Receiver<ReactionRequest>, pool: Arc<ConnectionPool>) {
    while let Some(req) = rx.recv().await {
        pool.send(req.to_addr, NetworkPacket::Reaction(req.event))
            .await;
    }
}

#[cfg(test)]
#[path = "../tests/test_network_sender.rs"]
mod tests;
