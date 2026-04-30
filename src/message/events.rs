use std::net::SocketAddr;

use super::chat::ChatMessage;
use super::group::GroupEvent;
use super::receipts::{MessageAck, ReadReceipt};

/// Événements réseau envoyés vers l'UI
#[derive(Clone, Debug)]
pub enum AppEvent {
    MessageReceived(ChatMessage),
    PeerDiscovered { username: String, addr: SocketAddr },
    PeerDisconnected { username: String },
    UserTyping(String),
    UserStoppedTyping(String),
    GroupEventReceived(GroupEvent),
    ReadReceiptReceived(ReadReceipt),
    MessageAckReceived(MessageAck),
}
