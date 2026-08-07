pub mod avatar;
pub mod chat;
pub mod events;
pub mod group;
pub mod media;
pub mod network_types;
pub mod reaction;
pub mod receipts;

pub use avatar::*;
pub use chat::*;
pub use events::*;
pub use group::*;
pub use media::*;
pub use network_types::*;
pub use reaction::*;
pub use receipts::*;

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Enveloppe réseau unifiée. Le champ `kind` (tag serde) permet de
/// désambiguïser tous les types de paquets, y compris ceux qui ont les
/// mêmes champs JSON (ReadReceipt vs MessageAck).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NetworkPacket {
    Chat(ChatMessage),
    Group(GroupEvent),
    Typing(TypingIndicator),
    ReadReceipt(ReadReceipt),
    Ack(MessageAck),
    Avatar(AvatarAnnounce),
    Reaction(ReactionEvent),
}

/// Commande sortante unique pour les paquets courts. Le streaming média garde
/// sa file dédiée car son cycle de vie et ses limites sont différents.
#[derive(Clone, Debug)]
pub struct NetworkSendRequest {
    pub to_peer: String,
    pub to_addr: SocketAddr,
    pub packet: NetworkPacket,
}

macro_rules! network_request_from {
    ($source:ty, $packet:expr) => {
        impl From<$source> for NetworkSendRequest {
            fn from(request: $source) -> Self {
                Self {
                    to_peer: request.to_peer.clone(),
                    to_addr: request.to_addr,
                    packet: $packet(request),
                }
            }
        }
    };
}

network_request_from!(SendRequest, |request: SendRequest| NetworkPacket::Chat(
    request.message
));
network_request_from!(SendGroupRequest, |request: SendGroupRequest| {
    NetworkPacket::Group(request.event)
});
network_request_from!(TypingRequest, |request: TypingRequest| {
    NetworkPacket::Typing(request.indicator)
});
network_request_from!(ReadReceiptRequest, |request: ReadReceiptRequest| {
    NetworkPacket::ReadReceipt(request.receipt)
});
network_request_from!(MessageAckRequest, |request: MessageAckRequest| {
    NetworkPacket::Ack(request.ack)
});
network_request_from!(AvatarRequest, |request: AvatarRequest| {
    NetworkPacket::Avatar(request.announce)
});
network_request_from!(ReactionRequest, |request: ReactionRequest| {
    NetworkPacket::Reaction(request.event)
});

#[cfg(test)]
#[path = "../tests/test_message_mod.rs"]
mod packet_tests;
