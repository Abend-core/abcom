pub mod avatar;
pub mod chat;
pub mod events;
pub mod group;
pub mod network_types;
pub mod receipts;

pub use avatar::*;
pub use chat::*;
pub use events::*;
pub use group::*;
pub use network_types::*;
pub use receipts::*;

use serde::{Deserialize, Serialize};

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
}
