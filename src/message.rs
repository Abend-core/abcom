use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Un message de chat sérialisé envoyé par TCP
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub from: String,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_user: Option<String>,  // None = broadcast, Some("Alice") = direct
}

/// Indicateur: quelqu'un est en train d'écrire
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TypingIndicator {
    pub from: String,
}

/// Paquet UDP pour la découverte des pairs sur le LAN
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiscoveryPacket {
    pub username: String,
}

/// Événements réseau envoyés vers l'UI
#[derive(Clone, Debug)]
pub enum AppEvent {
    MessageReceived(ChatMessage),
    PeerDiscovered { username: String, addr: SocketAddr },
    PeerDisconnected { username: String },  // Peer n'a pas répondu depuis trop longtemps
    UserTyping(String),  // nom d'utilisateur qui tape
    UserStoppedTyping(String),
}

/// Demande d'envoi d'un message à une adresse TCP
#[derive(Clone, Debug)]
pub struct SendRequest {
    pub to_addr: SocketAddr,
    pub message: ChatMessage,
}
