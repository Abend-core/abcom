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
    GroupEventReceived {
        peer: String,
        event: GroupEvent,
    },
    ReadReceiptReceived(ReadReceipt),
    MessageAckReceived(MessageAck),
    AvatarReceived(AvatarAnnounce),
    /// Début de réception d'un média : on crée le message (carte + progression).
    MediaIncoming(MediaStreamHeader),
    /// Progression d'un transfert média (émission ou réception).
    MediaProgressed(MediaProgress),
    /// Le destinataire a refusé un média : on l'annote dans le fil (côté émetteur).
    MediaDeclined {
        peer: String,
        header: MediaStreamHeader,
    },
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
        /// Clé effectivement présentée. Le ré-appairage épingle **celle-ci**,
        /// et pas la première clé reçue après un désépinglage — sinon une
        /// autre machine pourrait gagner la course.
        offered_key: Vec<u8>,
    },
    /// Résultats d'une recherche plein texte dans l'historique.
    SearchResults {
        query: String,
        messages: Vec<ChatMessage>,
    },
    /// Aucune connexion sécurisée n'a pu être établie vers ce pair : le
    /// paquet est perdu. Remonté à l'UI (bannière) car sur un binaire release
    /// sans console, l'utilisateur n'a aucun autre signal.
    SendFailed {
        username: String,
    },
    /// Fin d'une copie vers le dossier Téléchargements. Le travail se fait sur
    /// un thread dédié — un média peut peser plusieurs Gio et gèlerait l'UI —
    /// donc le verdict revient par événement.
    MediaDownloaded {
        /// Nom du fichier écrit, ou `None` si la copie a échoué.
        filename: Option<String>,
    },
}
