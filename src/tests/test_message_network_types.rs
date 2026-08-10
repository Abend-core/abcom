use super::{DiscoveryPacket, Hello, PeerRecord};

#[test]
fn peer_record_round_trip_with_alias() {
    let r = PeerRecord {
        username: "bob".to_string(),
        alias: Some("Robert".to_string()),
    };
    let json = serde_json::to_string(&r).unwrap();
    let decoded: PeerRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.username, "bob");
    assert_eq!(decoded.alias, Some("Robert".to_string()));
}

#[test]
fn peer_record_round_trip_no_alias() {
    let r = PeerRecord {
        username: "charlie".to_string(),
        alias: None,
    };
    let json = serde_json::to_string(&r).unwrap();
    let decoded: PeerRecord = serde_json::from_str(&json).unwrap();
    assert!(decoded.alias.is_none());
}

#[test]
fn discovery_packet_round_trip() {
    let p = DiscoveryPacket {
        username: "alice".to_string(),
        port: 9010,
        pubkey: "abcd".to_string(),
        ..Default::default()
    };
    let json = serde_json::to_string(&p).unwrap();
    let decoded: DiscoveryPacket = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.username, "alice");
    assert_eq!(decoded.port, 9010);
}

#[test]
fn discovery_packet_legacy_without_port_defaults_to_9000() {
    // Ancien paquet sans champ `port` → 9000 par défaut
    let json = r#"{"username":"bob"}"#;
    let decoded: DiscoveryPacket = serde_json::from_str(json).unwrap();
    assert_eq!(decoded.username, "bob");
    assert_eq!(decoded.port, 9000);
}

#[test]
fn peer_record_ignores_legacy_last_subnet_field() {
    // Anciens enregistrements avec `last_subnet` → champ inconnu ignoré
    let json = r#"{"username":"bob","alias":"Robert","last_subnet":"192.168.1"}"#;
    let decoded: PeerRecord = serde_json::from_str(json).unwrap();
    assert_eq!(decoded.username, "bob");
    assert_eq!(decoded.alias, Some("Robert".to_string()));
}

#[test]
fn hello_carries_protocol_version() {
    let hello = Hello {
        username: "alice".into(),
        protocol_version: crate::protocol::PROTOCOL_VERSION,
        capabilities: vec!["chat".into()],
    };
    let decoded: Hello = serde_json::from_str(&serde_json::to_string(&hello).unwrap()).unwrap();
    assert_eq!(decoded.protocol_version, crate::protocol::PROTOCOL_VERSION);
    assert_eq!(decoded.capabilities, vec!["chat"]);
}

#[test]
fn only_typing_indicators_may_be_dropped() {
    use crate::message::{
        AvatarAnnounce, ChatMessage, GroupAction, GroupEvent, MessageAck, NetworkPacket,
        ReactionEvent, ReadReceipt, TypingIndicator,
    };

    let typing = NetworkPacket::Typing(TypingIndicator {
        from: "alice".into(),
    });
    assert!(
        typing.is_droppable(),
        "la frappe est réémise en continu, sa perte est sans conséquence"
    );

    // Tout le reste doit attendre une place : l'interface affiche déjà ces
    // paquets comme partis.
    let critical = [
        NetworkPacket::Chat(ChatMessage {
            from: "alice".into(),
            content: "important".into(),
            timestamp: "12:00".into(),
            timestamp_epoch: Some(1),
            to_user: None,
            media: None,
            reply_to: None,
            nonce: None,
        }),
        NetworkPacket::Ack(MessageAck {
            from: "alice".into(),
            to: "bob".into(),
            message_hash: 1,
            timestamp: "12:00".into(),
        }),
        NetworkPacket::ReadReceipt(ReadReceipt {
            from: "alice".into(),
            to: "bob".into(),
            message_hash: 1,
            timestamp: "12:00".into(),
        }),
        NetworkPacket::Reaction(ReactionEvent {
            user: "alice".into(),
            message_hash: 1,
            emoji: "👍".into(),
            action: crate::message::ReactionAction::Add,
        }),
        NetworkPacket::Avatar(AvatarAnnounce {
            from: "alice".into(),
            png: Vec::new(),
        }),
        NetworkPacket::Group(GroupEvent {
            action: GroupAction::Delete {
                group_name: "projet".into(),
            },
        }),
    ];
    for packet in critical {
        assert!(
            !packet.is_droppable(),
            "paquet critique jetable : {packet:?}"
        );
    }
}
