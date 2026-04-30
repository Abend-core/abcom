use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Indicateur: quelqu'un est en train d'écrire
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TypingIndicator {
    pub from: String,
}

/// Demande d'envoi d'un indicateur de frappe
#[derive(Clone, Debug)]
pub struct TypingRequest {
    pub to_addr: SocketAddr,
    pub indicator: TypingIndicator,
}

/// Accusé de lecture d'un message
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReadReceipt {
    pub from: String,
    pub to: String,
    pub message_hash: u64,
    pub timestamp: String,
}

/// Demande d'envoi d'un accusé de lecture
#[derive(Clone, Debug)]
pub struct ReadReceiptRequest {
    pub to_addr: SocketAddr,
    pub receipt: ReadReceipt,
}

/// Accusé de réception d'un message (delivery ACK)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageAck {
    pub from: String,
    pub to: String,
    pub message_hash: u64,
    pub timestamp: String,
}

/// Demande d'envoi d'un ACK de livraison
#[derive(Clone, Debug)]
pub struct MessageAckRequest {
    pub to_addr: SocketAddr,
    pub ack: MessageAck,
}

/// Message réseau unifié (ChatMessage ou GroupEvent)
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum NetworkMessage {
    Chat(super::chat::ChatMessage),
    Group(super::group::GroupEvent),
}

#[cfg(test)]
mod tests {
    use super::{TypingIndicator, ReadReceipt, MessageAck, NetworkMessage};
    use crate::message::{ChatMessage, GroupEvent, GroupAction, Group};

    // ── TypingIndicator ─────────────────────────────────────────────────────

    #[test]
    fn typing_indicator_round_trip() {
        let t = TypingIndicator { from: "alice".to_string() };
        let json = serde_json::to_string(&t).unwrap();
        let decoded: TypingIndicator = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.from, "alice");
    }

    // ── ReadReceipt ─────────────────────────────────────────────────────────

    #[test]
    fn read_receipt_round_trip() {
        let r = ReadReceipt {
            from: "bob".to_string(),
            to: "alice".to_string(),
            message_hash: 123456789,
            timestamp: "14:00".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let decoded: ReadReceipt = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.from, "bob");
        assert_eq!(decoded.to, "alice");
        assert_eq!(decoded.message_hash, 123456789);
        assert_eq!(decoded.timestamp, "14:00");
    }

    #[test]
    fn read_receipt_hash_preserves_zero() {
        let r = ReadReceipt { from: "a".to_string(), to: "b".to_string(), message_hash: 0, timestamp: "".to_string() };
        let json = serde_json::to_string(&r).unwrap();
        let decoded: ReadReceipt = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.message_hash, 0);
    }

    // ── MessageAck ──────────────────────────────────────────────────────────

    #[test]
    fn message_ack_round_trip() {
        let a = MessageAck {
            from: "alice".to_string(),
            to: "bob".to_string(),
            message_hash: 987654321,
            timestamp: "15:30".to_string(),
        };
        let json = serde_json::to_string(&a).unwrap();
        let decoded: MessageAck = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.from, "alice");
        assert_eq!(decoded.message_hash, 987654321);
    }

    // ── NetworkMessage untagged dispatch ────────────────────────────────────

    #[test]
    fn network_message_dispatches_chat() {
        let chat = ChatMessage {
            from: "alice".to_string(),
            content: "bonjour".to_string(),
            timestamp: "10:00".to_string(),
            to_user: None,
        };
        let json = serde_json::to_string(&chat).unwrap();
        let nm: NetworkMessage = serde_json::from_str(&json).unwrap();
        match nm {
            NetworkMessage::Chat(m) => assert_eq!(m.content, "bonjour"),
            NetworkMessage::Group(_) => panic!("Devrait être Chat"),
        }
    }

    #[test]
    fn network_message_dispatches_group_event() {
        let event = GroupEvent {
            action: GroupAction::Delete { group_name: "Team".to_string() },
        };
        let json = serde_json::to_string(&event).unwrap();
        let nm: NetworkMessage = serde_json::from_str(&json).unwrap();
        match nm {
            NetworkMessage::Group(e) => match e.action {
                GroupAction::Delete { group_name } => assert_eq!(group_name, "Team"),
                _ => panic!("Mauvais variant"),
            },
            NetworkMessage::Chat(_) => panic!("Devrait être Group"),
        }
    }
}
