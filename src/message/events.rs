use std::net::SocketAddr;

use super::avatar::AvatarAnnounce;
use super::chat::ChatMessage;
use super::group::GroupEvent;
use super::media::{MediaProgress, MediaStreamHeader};
use super::reaction::ReactionEvent;
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
    GroupEventReceived(GroupEvent),
    ReadReceiptReceived(ReadReceipt),
    MessageAckReceived(MessageAck),
    AvatarReceived(AvatarAnnounce),
    /// Début de réception d'un média : on crée le message (carte + progression).
    MediaIncoming(MediaStreamHeader),
    /// Progression d'un transfert média (émission ou réception).
    MediaProgressed(MediaProgress),
    /// Le destinataire a refusé un média : on l'annote dans le fil (côté émetteur).
    MediaDeclined(MediaStreamHeader),
    /// Un pair a ajouté ou retiré une réaction emoji sur un message.
    ReactionReceived(ReactionEvent),
    /// Page d'historique plus ancienne chargée depuis SQLite (pagination du
    /// fil vers le haut). `oldest_rowid` = None si le début est atteint.
    OlderMessagesLoaded {
        messages: Vec<ChatMessage>,
        oldest_rowid: Option<i64>,
    },
    /// La clé statique présentée par ce pair ne correspond pas à celle
    /// épinglée (TOFU) : la connexion a été refusée, l'utilisateur doit être
    /// prévenu d'une possible usurpation.
    KeyChanged {
        username: String,
    },
}
