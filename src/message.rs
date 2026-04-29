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

/// Réseau connu (identifié par SSID WiFi si disponible, sinon subnet)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KnownNetwork {
    /// Identifiant unique: SSID WiFi si disponible, sinon subnet (ex: "iPhone de Hugo" ou "192.168.1")
    /// Champ avec default="" pour compatibilité avec anciens networks.json (migration faite au chargement)
    #[serde(default)]
    pub id: String,
    /// Subnet IP associé (ex: "192.168.1") — info secondaire, non utilisé comme clé primaire
    #[serde(default)]
    pub subnet: String,
    /// Alias optionnel donné par l'utilisateur
    pub alias: Option<String>,
    /// Noms des pairs vus sur ce réseau
    pub seen_peers: Vec<String>,
}

impl KnownNetwork {
    pub fn display_name(&self) -> String {
        if let Some(alias) = &self.alias {
            alias.clone()
        } else if !self.id.is_empty() {
            self.id.clone()
        } else {
            format!("{}.x", self.subnet)
        }
    }
}

/// Alias donné à un pair
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerRecord {
    pub username: String,
    pub alias: Option<String>,
    /// Subnet du réseau sur lequel ce pair a été vu en dernier
    pub last_subnet: Option<String>,
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
    ReadReceiptReceived(ReadReceipt),
    MessageAckReceived(MessageAck),
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

/// Request for typing indicator (sent when user is typing)
#[derive(Clone, Debug)]
pub struct TypingRequest {
    pub to_addr: SocketAddr,
    pub indicator: TypingIndicator,
}

/// Read receipt: confirmation that a message was read
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadReceipt {
    pub from: String,
    pub to: String,
    pub message_hash: u64,  // Hash of message for identification
    pub timestamp: String,
}

/// Request for sending read receipt to a peer
#[derive(Clone, Debug)]
pub struct ReadReceiptRequest {
    pub to_addr: SocketAddr,
    pub receipt: ReadReceipt,
}

/// Message acknowledgment: confirmation that a message was received
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageAck {
    pub from: String,
    pub to: String,
    pub message_hash: u64,
    pub timestamp: String,
}

/// Request for sending message ACK to a peer
#[derive(Clone, Debug)]
pub struct MessageAckRequest {
    pub to_addr: SocketAddr,
    pub ack: MessageAck,
}

/// Message réseau unifié (ChatMessage ou GroupEvent)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum NetworkMessage {
    Chat(ChatMessage),
    Group(GroupEvent),
}
