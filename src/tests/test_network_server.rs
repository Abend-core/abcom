use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::identity::Identity;
use crate::message::{
    AppEvent, AvatarAnnounce, ChatMessage, GroupAction, GroupEvent, MessageAck, NetworkPacket,
    ReactionAction, ReactionEvent, ReadReceipt, TypingIndicator,
};
use crate::network::secure::{exchange_hello, handshake_initiator, SecureStream, TrustStore};
use crate::network::NetContext;

fn ctx(username: &str, tx: mpsc::Sender<AppEvent>) -> Arc<NetContext> {
    Arc::new(NetContext {
        identity: Identity::ephemeral().unwrap(),
        username: username.to_string(),
        trust: Arc::new(TrustStore::new(Default::default(), None)),
        event_tx: tx,
        psk: None,
    })
}

/// Client de test : connexion chiffrée + Hello, prêt à émettre des paquets.
async fn secure_client(addr: std::net::SocketAddr, username: &str) -> SecureStream {
    secure_client_with_identity(addr, username, &Identity::ephemeral().unwrap()).await
}

async fn secure_client_with_identity(
    addr: std::net::SocketAddr,
    username: &str,
    identity: &Identity,
) -> SecureStream {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (transport, _) = handshake_initiator(&mut stream, identity, None)
        .await
        .unwrap();
    let mut secure = SecureStream::new(stream, transport);
    exchange_hello(&mut secure, username, true).await.unwrap();
    secure
}

#[tokio::test]
async fn duplicate_authenticated_session_is_rejected() {
    let (tx, mut rx) = mpsc::channel(8);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(super::run_server_on(listener, ctx("serveur", tx)));

    let identity = Identity::ephemeral().unwrap();
    let mut first = secure_client_with_identity(addr, "alice", &identity).await;
    let packet = NetworkPacket::Ack(MessageAck {
        from: "alice".into(),
        to: "serveur".into(),
        message_hash: 1,
        timestamp: "12:00".into(),
    });
    first
        .send(&serde_json::to_vec(&packet).unwrap())
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap(),
        Some(AppEvent::MessageAckReceived(_))
    ));

    let mut second = secure_client_with_identity(addr, "alice", &identity).await;
    let _ = second.send(&serde_json::to_vec(&packet).unwrap()).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(300), rx.recv())
            .await
            .is_err(),
        "la seconde session ne doit produire aucun événement"
    );
}

async fn dispatch(packet: NetworkPacket) -> Option<AppEvent> {
    let (tx, mut rx) = mpsc::channel(4);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_ctx = ctx("serveur", tx);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let _ = super::handle_incoming(stream, server_ctx).await;
    });

    let mut client = secure_client(addr, "client").await;
    client
        .send(&serde_json::to_vec(&packet).unwrap())
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .ok()
        .flatten()
}

#[tokio::test]
async fn test_receives_chat_message() {
    let packet = NetworkPacket::Chat(ChatMessage {
        from: "client".to_string(),
        content: "hello".to_string(),
        timestamp: "14:00".to_string(),
        timestamp_epoch: None,
        to_user: None,
        media: None,
        reply_to: None,
        nonce: None,
    });
    let event = dispatch(packet).await.unwrap();
    assert!(matches!(event, AppEvent::MessageReceived(m) if m.content == "hello"));
}

#[tokio::test]
async fn test_receives_read_receipt() {
    let packet = NetworkPacket::ReadReceipt(ReadReceipt {
        from: "client".to_string(),
        to: "serveur".to_string(),
        message_hash: 42,
        timestamp: "14:00".to_string(),
    });
    let event = dispatch(packet).await.unwrap();
    assert!(matches!(event, AppEvent::ReadReceiptReceived(r) if r.from == "client"));
}

#[tokio::test]
async fn test_receives_ack() {
    let packet = NetworkPacket::Ack(MessageAck {
        from: "client".to_string(),
        to: "serveur".to_string(),
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
        user: "client".to_string(),
        action: ReactionAction::Add,
    });
    let event = dispatch(packet).await.unwrap();
    assert!(matches!(
        event,
        AppEvent::ReactionReceived(e) if e.message_hash == 55 && e.action == ReactionAction::Add
    ));
}

#[tokio::test]
async fn test_multiple_packets_on_one_connection() {
    // Connexion persistante : plusieurs paquets passent sur la même session.
    let (tx, mut rx) = mpsc::channel(8);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_ctx = ctx("serveur", tx);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let _ = super::handle_incoming(stream, server_ctx).await;
    });

    let mut client = secure_client(addr, "client").await;
    for i in 0..3 {
        let packet = NetworkPacket::Ack(MessageAck {
            from: "client".to_string(),
            to: "serveur".to_string(),
            message_hash: i,
            timestamp: "14:00".to_string(),
        });
        client
            .send(&serde_json::to_vec(&packet).unwrap())
            .await
            .unwrap();
    }
    for i in 0..3 {
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, AppEvent::MessageAckReceived(a) if a.message_hash == i));
    }
}

#[tokio::test]
async fn test_large_avatar_packet_passes() {
    // Régression du bug « paquet > 64 Ko rejeté » : une annonce d'avatar
    // volumineuse traverse le framing chiffré multi-frames.
    let packet = NetworkPacket::Avatar(crate::message::AvatarAnnounce {
        from: "client".to_string(),
        png: vec![42u8; 150_000],
    });
    let event = dispatch(packet).await.unwrap();
    assert!(matches!(event, AppEvent::AvatarReceived(a) if a.png.len() == 150_000));
}

#[tokio::test]
async fn test_forged_authors_and_wrong_recipients_are_rejected() {
    let (tx, mut rx) = mpsc::channel(16);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_ctx = ctx("serveur", tx);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let _ = super::handle_incoming(stream, server_ctx).await;
    });

    let mut client = secure_client(addr, "client").await;
    let forged = [
        NetworkPacket::Chat(ChatMessage {
            from: "mallory".to_string(),
            content: "forged".to_string(),
            timestamp: "14:00".to_string(),
            timestamp_epoch: None,
            to_user: None,
            media: None,
            reply_to: None,
            nonce: None,
        }),
        NetworkPacket::Typing(TypingIndicator {
            from: "mallory".to_string(),
        }),
        NetworkPacket::ReadReceipt(ReadReceipt {
            from: "mallory".to_string(),
            to: "serveur".to_string(),
            message_hash: 1,
            timestamp: "14:00".to_string(),
        }),
        NetworkPacket::Ack(MessageAck {
            from: "mallory".to_string(),
            to: "serveur".to_string(),
            message_hash: 1,
            timestamp: "14:00".to_string(),
        }),
        NetworkPacket::Avatar(AvatarAnnounce {
            from: "mallory".to_string(),
            png: Vec::new(),
        }),
        NetworkPacket::Reaction(ReactionEvent {
            message_hash: 1,
            emoji: "x".to_string(),
            user: "mallory".to_string(),
            action: ReactionAction::Add,
        }),
        NetworkPacket::Chat(ChatMessage {
            from: "client".to_string(),
            content: "wrong recipient".to_string(),
            timestamp: "14:00".to_string(),
            timestamp_epoch: None,
            to_user: Some("quelqu-un-d-autre".to_string()),
            media: None,
            reply_to: None,
            nonce: None,
        }),
        NetworkPacket::ReadReceipt(ReadReceipt {
            from: "client".to_string(),
            to: "quelqu-un-d-autre".to_string(),
            message_hash: 1,
            timestamp: "14:00".to_string(),
        }),
        NetworkPacket::Ack(MessageAck {
            from: "client".to_string(),
            to: "quelqu-un-d-autre".to_string(),
            message_hash: 1,
            timestamp: "14:00".to_string(),
        }),
    ];
    for packet in forged {
        client
            .send(&serde_json::to_vec(&packet).unwrap())
            .await
            .unwrap();
    }
    client
        .send(
            &serde_json::to_vec(&NetworkPacket::Typing(TypingIndicator {
                from: "client".to_string(),
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(event, AppEvent::UserTyping(ref peer) if peer == "client"));
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn test_group_event_preserves_authenticated_peer() {
    let event = dispatch(NetworkPacket::Group(GroupEvent {
        action: GroupAction::Delete {
            group_name: "Team".to_string(),
        },
    }))
    .await
    .unwrap();

    assert!(matches!(
        event,
        AppEvent::GroupEventReceived { peer, .. } if peer == "client"
    ));
}

#[tokio::test]
async fn test_invalid_json_dispatches_nothing() {
    let (tx, mut rx) = mpsc::channel(4);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_ctx = ctx("serveur", tx);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let _ = super::handle_incoming(stream, server_ctx).await;
    });

    let mut client = secure_client(addr, "client").await;
    client.send(b"NOT_VALID_JSON{{{").await.unwrap();

    let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(
        event.is_err(),
        "un paquet invalide ne produit aucun événement"
    );
}

#[tokio::test]
async fn test_plaintext_client_is_rejected() {
    // Un client non chiffré (ancien protocole) ne peut pas parler au serveur.
    let (tx, mut rx) = mpsc::channel(4);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_ctx = ctx("serveur", tx);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let _ = super::handle_incoming(stream, server_ctx).await;
    });

    use tokio::io::AsyncWriteExt;
    let mut client = TcpStream::connect(addr).await.unwrap();
    client
        .write_all(br#"{"kind":"chat","from":"mallory","content":"pwn","timestamp":"14:00"}"#)
        .await
        .unwrap();
    client.shutdown().await.unwrap();

    // Le handshake échoue : soit aucun événement (timeout), soit le canal se
    // ferme (None) quand la connexion est abandonnée — jamais de paquet.
    let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
    assert!(
        event.is_err() || event.unwrap().is_none(),
        "le clair doit être rejeté au handshake"
    );
}

#[tokio::test]
async fn test_key_change_is_refused_and_reported() {
    // TOFU : le serveur épingle la clé de « client » à la première session ;
    // une seconde session avec une autre clé mais le même nom est refusée.
    let (tx, mut rx) = mpsc::channel(8);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server_ctx = ctx("serveur", tx);

    {
        let server_ctx = server_ctx.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let ctx = server_ctx.clone();
                tokio::spawn(async move {
                    let _ = super::handle_incoming(stream, ctx).await;
                });
            }
        });
    }

    // Première session : clé épinglée, le paquet passe.
    let mut first = secure_client(addr, "client").await;
    let ack = NetworkPacket::Ack(MessageAck {
        from: "client".to_string(),
        to: "serveur".to_string(),
        message_hash: 1,
        timestamp: "14:00".to_string(),
    });
    first
        .send(&serde_json::to_vec(&ack).unwrap())
        .await
        .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(event, AppEvent::MessageAckReceived(_)));

    // Seconde session : même username, identité différente → alerte + refus.
    let mut second = secure_client(addr, "client").await;
    let _ = second.send(&serde_json::to_vec(&ack).unwrap()).await;
    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(event, AppEvent::KeyChanged { ref username } if username == "client"),
        "alerte de changement de clé attendue"
    );
}
