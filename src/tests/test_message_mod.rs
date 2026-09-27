use super::*;

fn chat() -> ChatMessage {
    ChatMessage {
        from: "bob".to_string(),
        content: "salut".to_string(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: Some(1_750_000_000),
        to_user: Some("ellis".to_string()),
        media: None,
        reply_to: None,
        nonce: None,
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

#[test]
fn network_packet_reaction_round_trip() {
    let packet = NetworkPacket::Reaction(ReactionEvent {
        message_hash: 7,
        emoji: "❤️".to_string(),
        user: "bob".to_string(),
        action: ReactionAction::Add,
    });
    let json = serde_json::to_string(&packet).unwrap();
    let decoded: NetworkPacket = serde_json::from_str(&json).unwrap();
    match decoded {
        NetworkPacket::Reaction(e) => {
            assert_eq!(e.message_hash, 7);
            assert_eq!(e.user, "bob");
            assert_eq!(e.action, ReactionAction::Add);
        }
        _ => panic!("devrait être Reaction, json = {json}"),
    }
}
