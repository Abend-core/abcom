
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

use crate::message::{ChatMessage, NetworkPacket, ReactionAction, ReactionEvent};

#[tokio::test]
async fn test_send_packet_delivers_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let packet = NetworkPacket::Chat(ChatMessage {
        from: "alice".to_string(),
        content: "test send".to_string(),
        timestamp: "10:00".to_string(),
        timestamp_epoch: None,
        to_user: Some("bob".to_string()),
        media: None,
        reply_to: None,
    });
    let expected = serde_json::to_vec(&packet).unwrap();

    let recv = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        buf
    });

    super::send_packet(addr, packet).await;

    let received = tokio::time::timeout(Duration::from_secs(2), recv)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(received, expected);
}

#[tokio::test]
async fn test_send_packet_deserializes_correctly() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let packet = NetworkPacket::Chat(ChatMessage {
        from: "rudy".to_string(),
        content: "héllo 🎉".to_string(),
        timestamp: "14:30".to_string(),
        timestamp_epoch: Some(1_750_000_000),
        to_user: None,
        media: None,
        reply_to: None,
    });

    let recv = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        serde_json::from_slice::<NetworkPacket>(&buf).unwrap()
    });

    super::send_packet(addr, packet).await;

    let decoded = tokio::time::timeout(Duration::from_secs(2), recv)
        .await
        .unwrap()
        .unwrap();

    match decoded {
        NetworkPacket::Chat(m) => {
            assert_eq!(m.from, "rudy");
            assert_eq!(m.content, "héllo 🎉");
            assert_eq!(m.timestamp_epoch, Some(1_750_000_000));
        }
        _ => panic!("Mauvais type de paquet reçu"),
    }
}

#[tokio::test]
async fn test_send_packet_reaction_deserializes_correctly() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let packet = NetworkPacket::Reaction(ReactionEvent {
        message_hash: 123,
        emoji: "😂".to_string(),
        user: "rudy".to_string(),
        action: ReactionAction::Remove,
    });

    let recv = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        serde_json::from_slice::<NetworkPacket>(&buf).unwrap()
    });

    super::send_packet(addr, packet).await;

    let decoded = tokio::time::timeout(Duration::from_secs(2), recv)
        .await
        .unwrap()
        .unwrap();

    match decoded {
        NetworkPacket::Reaction(e) => {
            assert_eq!(e.message_hash, 123);
            assert_eq!(e.action, ReactionAction::Remove);
        }
        _ => panic!("Mauvais type de paquet reçu"),
    }
}

#[tokio::test]
async fn test_send_packet_connection_refused_does_not_panic() {
    let addr: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
    let packet = NetworkPacket::Chat(ChatMessage {
        from: "alice".to_string(),
        content: "unreachable".to_string(),
        timestamp: "10:00".to_string(),
        timestamp_epoch: None,
        to_user: None,
        media: None,
        reply_to: None,
    });
    super::send_packet(addr, packet).await;
}
