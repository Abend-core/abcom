use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Un message de chat sérialisé envoyé par TCP
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub from: String,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_user: Option<String>,  // None = broadcast, Some("Alice") = direct, Some("#GroupName") = group
}

/// Représente un groupe de chat
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Group {
    pub name: String,
    pub owner: String,  // Celui qui a créé le groupe
    pub members: Vec<String>,  // Noms des membres
    pub created_at: String,
}

/// Événement de synchronisation de groupe envoyé par TCP
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GroupEvent {
    pub action: GroupAction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GroupAction {
    Create { group: Group },
    AddMember { group_name: String, username: String },
    RemoveMember { group_name: String, username: String },
    Rename { group_name: String, new_name: String },
    Delete { group_name: String },
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
    GroupEventReceived(GroupEvent),
}

/// Demande d'envoi d'un message à une adresse TCP
#[derive(Clone, Debug)]
pub struct SendRequest {
    pub to_addr: SocketAddr,
    pub message: ChatMessage,
}

/// Demande d'envoi d'un événement de groupe à une adresse TCP
#[derive(Clone, Debug)]
pub struct SendGroupRequest {
    pub to_addr: SocketAddr,
    pub event: GroupEvent,
}

/// Demande d'envoi d'un indicateur de frappe à une adresse TCP
#[derive(Clone, Debug)]
pub struct SendTypingRequest {
    pub to_addr: SocketAddr,
    pub from: String,
}

/// Message réseau unifié (ChatMessage ou GroupEvent)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum NetworkMessage {
    Chat(ChatMessage),
    Group(GroupEvent),
}
