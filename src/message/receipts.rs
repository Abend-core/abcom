use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Indicateur: quelqu'un est en train d'écrire
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TypingIndicator {
    pub from: String,
}

/// Demande d'envoi d'un indicateur de frappe
#[derive(Clone, Debug)]
pub struct TypingRequest {
    pub to_addr: SocketAddr,
    pub indicator: TypingIndicator,
}

/// Accusé de lecture d'un message
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadReceipt {
    pub from: String,
    pub to: String,
    pub message_hash: u64,
    pub timestamp: String,
}

/// Demande d'envoi d'un accusé de lecture
#[derive(Clone, Debug)]
pub struct ReadReceiptRequest {
    pub to_addr: SocketAddr,
    pub receipt: ReadReceipt,
}

/// Accusé de réception d'un message (delivery ACK)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageAck {
    pub from: String,
    pub to: String,
    pub message_hash: u64,
    pub timestamp: String,
}

/// Demande d'envoi d'un ACK de livraison
#[derive(Clone, Debug)]
pub struct MessageAckRequest {
    pub to_addr: SocketAddr,
    pub ack: MessageAck,
}

/// Message réseau unifié (ChatMessage ou GroupEvent)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum NetworkMessage {
    Chat(super::chat::ChatMessage),
    Group(super::group::GroupEvent),
}
