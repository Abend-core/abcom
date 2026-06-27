pub mod avatar;
pub mod chat;
pub mod events;
pub mod group;
pub mod media;
pub mod network_types;
pub mod receipts;

pub use avatar::*;
pub use chat::*;
pub use events::*;
pub use group::*;
pub use media::*;
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

#[cfg(test)]
mod packet_tests {
    use super::*;

    fn chat() -> ChatMessage {
        ChatMessage {
            from: "bob".to_string(),
            content: "salut".to_string(),
            timestamp: "12:00".to_string(),
            timestamp_epoch: Some(1_750_000_000),
            to_user: Some("ellis".to_string()),
            media: None,
        }
    }

    #[test]
    fn network_packet_chat_round_trip() {
        let packet = NetworkPacket::Chat(chat());
        let json = serde_json::to_string(&packet).unwrap();
        let decoded: NetworkPacket = serde_json::from_str(&json).unwrap();
        match decoded {
            NetworkPacket::Chat(m) => {
                assert_eq!(m.content, "salut");
                assert_eq!(m.to_user.as_deref(), Some("ellis"));
            }
            _ => panic!("devrait être Chat, json = {json}"),
        }
    }
}
