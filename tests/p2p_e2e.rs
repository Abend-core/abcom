use std::sync::Arc;
use std::time::Duration;

use abcom::identity::Identity;
use abcom::message::{AppEvent, ChatMessage, NetworkPacket, NetworkSendRequest};
use abcom::network::secure::TrustStore;
use abcom::network::{run_sender, run_server_on, ConnectionPool, NetContext};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

fn context(username: &str, event_tx: mpsc::Sender<AppEvent>) -> Arc<NetContext> {
    Arc::new(NetContext {
        identity: Identity::ephemeral().unwrap(),
        username: username.to_string(),
        trust: Arc::new(TrustStore::new(Default::default(), None)),
        event_tx,
        psk: None,
    })
}

#[tokio::test]
async fn two_headless_peers_exchange_an_authenticated_message() {
    let (bob_events_tx, mut bob_events_rx) = mpsc::channel(8);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bob_addr = listener.local_addr().unwrap();
    tokio::spawn(run_server_on(listener, context("bob", bob_events_tx)));

    let (alice_events_tx, _alice_events_rx) = mpsc::channel(8);
    let alice_pool = ConnectionPool::new(context("alice", alice_events_tx));
    let (send_tx, send_rx) = mpsc::channel(8);
    tokio::spawn(run_sender(send_rx, alice_pool));
    send_tx
        .send(NetworkSendRequest {
            to_peer: "bob".into(),
            to_addr: bob_addr,
            packet: NetworkPacket::Chat(ChatMessage {
                from: "alice".into(),
                content: "bonjour bob".into(),
                timestamp: "12:00".into(),
                timestamp_epoch: Some(1_750_000_000),
                to_user: Some("bob".into()),
                media: None,
                reply_to: None,
                nonce: Some(1),
            }),
        })
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(3), bob_events_rx.recv())
        .await
        .expect("message P2P reçu avant le timeout")
        .expect("canal d'événements ouvert");
    assert!(matches!(
        received,
        AppEvent::MessageReceived(message)
            if message.from == "alice" && message.content == "bonjour bob"
    ));
}
