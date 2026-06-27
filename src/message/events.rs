use std::net::SocketAddr;

use super::avatar::AvatarAnnounce;
use super::chat::ChatMessage;
use super::group::GroupEvent;
use super::media::{MediaProgress, MediaStreamHeader};
use super::receipts::{MessageAck, ReadReceipt};

/// Événements réseau envoyés vers l'UI
#[derive(Clone, Debug)]
pub enum AppEvent {
    MessageReceived(ChatMessage),
    PeerDiscovered {
        username: String,
        addr: SocketAddr,
    },
    PeerDisconnected {
        username: String,
    },
    UserTyping(String),
    #[allow(dead_code)]
    UserStoppedTyping(String),
    GroupEventReceived(GroupEvent),
    ReadReceiptReceived(ReadReceipt),
    MessageAckReceived(MessageAck),
    AvatarReceived(AvatarAnnounce),
    /// Début de réception d'un média : on crée le message (carte + progression).
    MediaIncoming(MediaStreamHeader),
    /// Progression d'un transfert média (émission ou réception).
    MediaProgressed(MediaProgress),
}
