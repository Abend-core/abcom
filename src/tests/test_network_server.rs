
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::message::{
    AppEvent, ChatMessage, MessageAck, NetworkPacket, ReactionAction, ReactionEvent, ReadReceipt,
};

async fn dispatch(packet: NetworkPacket) -> Option<AppEvent> {
    let (tx, mut rx) = mpsc::channel(4);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        super::handle_incoming(stream, tx).await;
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    client
        .write_all(&serde_json::to_vec(&packet).unwrap())
        .await
        .unwrap();
    client.shutdown().await.unwrap();

    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .ok()
        .flatten()
}

#[tokio::test]
async fn test_receives_chat_message() {
    let packet = NetworkPacket::Chat(ChatMessage {
        from: "alice".to_string(),
        content: "hello".to_string(),
        timestamp: "14:00".to_string(),
        timestamp_epoch: None,
        to_user: None,
        media: None,
        reply_to: None,
    });
    let event = dispatch(packet).await.unwrap();
    assert!(matches!(event, AppEvent::MessageReceived(m) if m.content == "hello"));
}

#[tokio::test]
async fn test_receives_read_receipt() {
    let packet = NetworkPacket::ReadReceipt(ReadReceipt {
        from: "bob".to_string(),
        to: "alice".to_string(),
        message_hash: 42,
        timestamp: "14:00".to_string(),
    });
    let event = dispatch(packet).await.unwrap();
    assert!(matches!(event, AppEvent::ReadReceiptReceived(r) if r.from == "bob"));
}

#[tokio::test]
async fn test_receives_ack() {
    let packet = NetworkPacket::Ack(MessageAck {
        from: "bob".to_string(),
        to: "alice".to_string(),
        message_hash: 99,
        timestamp: "14:00".to_string(),
    });
    let event = dispatch(packet).await.unwrap();
    assert!(matches!(event, AppEvent::MessageAckReceived(a) if a.message_hash == 99));
}

#[tokio::test]
async fn test_receives_reaction() {
    let packet = NetworkPacket::Reaction(ReactionEvent {
        message_hash: 55,
        emoji: "👍".to_string(),
        user: "bob".to_string(),
        action: ReactionAction::Add,
    });
    let event = dispatch(packet).await.unwrap();
    assert!(matches!(
        event,
        AppEvent::ReactionReceived(e) if e.message_hash == 55 && e.action == ReactionAction::Add
    ));
}

#[tokio::test]
async fn test_invalid_packet_dispatches_nothing() {
    let (tx, mut rx) = mpsc::channel(4);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        super::handle_incoming(stream, tx).await;
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(b"NOT_VALID_JSON{{{").await.unwrap();
    client.shutdown().await.unwrap();

    let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(event.is_err() || event.unwrap().is_none());
}

#[tokio::test]
async fn test_oversized_packet_dispatches_nothing() {
    let (tx, mut rx) = mpsc::channel(4);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        super::handle_incoming(stream, tx).await;
    });

    let mut client = TcpStream::connect(addr).await.unwrap();
    let oversized = vec![b'x'; super::MAX_PACKET_SIZE as usize + 1];
    client.write_all(&oversized).await.unwrap();
    client.shutdown().await.unwrap();

    let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(event.is_err() || event.unwrap().is_none());
}
