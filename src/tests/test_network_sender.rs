use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::identity::Identity;
use crate::message::{AppEvent, ChatMessage, NetworkPacket};
use crate::network::secure::TrustStore;
use crate::network::{ConnectionPool, NetContext};

fn ctx(username: &str, tx: mpsc::Sender<AppEvent>) -> Arc<NetContext> {
    Arc::new(NetContext {
        identity: Identity::ephemeral().unwrap(),
        username: username.to_string(),
        trust: Arc::new(TrustStore::new(Default::default(), None)),
        event_tx: tx,
    })
}

fn chat(content: &str) -> NetworkPacket {
    NetworkPacket::Chat(ChatMessage {
        from: "alice".to_string(),
        content: content.to_string(),
        timestamp: "14:00".to_string(),
        timestamp_epoch: None,
        to_user: None,
        media: None,
        reply_to: None,
    })
}

#[tokio::test]
async fn pool_sends_over_persistent_encrypted_connection() {
    let (tx, mut rx) = mpsc::channel(8);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Serveur réel (handshake + dispatch) sur le port éphémère.
    let server_ctx = ctx("serveur", tx.clone());
    tokio::spawn(async move {
        crate::network::run_server_on(listener, server_ctx).await;
    });

    let (client_tx, mut client_rx) = mpsc::channel(4);
    let client_ctx = ctx("alice", client_tx);
    let pool = ConnectionPool::new(client_ctx);

    // Deux envois : le second réutilise la connexion établie par le premier.
    pool.send(addr, chat("premier")).await;
    pool.send(addr, chat("second")).await;

    for expected in ["premier", "second"] {
        let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, AppEvent::MessageReceived(m) if m.content == expected));
    }
    // Aucune alerte côté client.
    assert!(client_rx.try_recv().is_err());
}
