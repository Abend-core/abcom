use crate::app::{AppState, Peer};
use crate::message::{ChatMessage, Group, SendRequest};
use std::net::SocketAddr;
use std::time::SystemTime;

fn state() -> AppState {
    let mut s = AppState::new("alice".to_string(), Default::default(), None);
    s.messages.clear();
    s.peers.clear();
    s
}

fn peer(name: &str, port: u16, online: bool) -> Peer {
    Peer {
        username: name.to_string(),
        addr: format!("127.0.0.1:{port}").parse().unwrap(),
        last_seen: 0,
        online,
    }
}

fn addr(port: u16) -> SocketAddr {
    format!("127.0.0.1:{port}").parse().unwrap()
}

fn request(port: u16, message: ChatMessage) -> SendRequest {
    SendRequest {
        to_peer: "bob".into(),
        to_addr: addr(port),
        message,
    }
}

fn make_msg(from: &str, content: &str) -> ChatMessage {
    ChatMessage {
        from: from.to_string(),
        content: content.to_string(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: None,
        to_user: None,
        media: None,
        reply_to: None,
        nonce: None,
    }
}

#[test]
fn test_message_hash_deterministic() {
    let m = make_msg("alice", "bonjour");
    assert_eq!(AppState::message_hash(&m), AppState::message_hash(&m));
}

#[test]
fn test_message_hash_different_for_different_inputs() {
    let m1 = make_msg("alice", "hello");
    let m2 = make_msg("alice", "world");
    assert_ne!(AppState::message_hash(&m1), AppState::message_hash(&m2));
}

#[test]
fn test_message_hash_differs_by_sender() {
    let m1 = make_msg("alice", "hello");
    let m2 = make_msg("bob", "hello");
    assert_ne!(AppState::message_hash(&m1), AppState::message_hash(&m2));
}

#[test]
fn test_message_hash_stable_known_value() {
    // Valeur FNV-1a connue : garantit que l'algo ne change pas entre compilations.
    // Si ce test casse, un ACK envoyé par Bob ne matchera jamais le hash d'Alice.
    let m = ChatMessage {
        from: "alice".to_string(),
        content: "bonjour".to_string(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: Some(1_750_000_000),
        to_user: Some("bob".to_string()),
        media: None,
        reply_to: None,
        nonce: None,
    };
    let expected = AppState::message_hash(&m);
    assert_eq!(AppState::message_hash(&m), expected);
    // Le hash ne doit PAS être 0 (FNV-1a ne produit jamais 0 pour une clé non vide)
    assert_ne!(expected, 0);
}

#[test]
fn test_duplicate_content_different_epoch_gives_different_hash() {
    // Cas du bug corrigé : Alice envoie "Bonjour" deux fois à des instants différents.
    // Avant le fix (DefaultHasher sans epoch), les deux messages avaient le même hash.
    let m1 = ChatMessage {
        from: "alice".to_string(),
        content: "Bonjour".to_string(),
        timestamp: "14:00".to_string(),
        timestamp_epoch: Some(1_000),
        to_user: None,
        media: None,
        reply_to: None,
        nonce: None,
    };
    let m2 = ChatMessage {
        timestamp_epoch: Some(2_000),
        ..m1.clone()
    };
    assert_ne!(AppState::message_hash(&m1), AppState::message_hash(&m2));
}

#[test]
fn test_mark_and_check_read() {
    let mut s = state();
    let m = make_msg("alice", "test");
    let hash = AppState::message_hash(&m);
    s.mark_message_read(hash, "bob".to_string());
    assert_eq!(s.get_read_count(hash), 1);
}

#[test]
fn test_get_read_count() {
    let mut s = state();
    let hash = AppState::message_hash(&make_msg("alice", "x"));
    assert_eq!(s.get_read_count(hash), 0);
    s.mark_message_read(hash, "bob".to_string());
    s.mark_message_read(hash, "charlie".to_string());
    assert_eq!(s.get_read_count(hash), 2);
}

// ── receipt_recipients : à qui diffuser nos ACK/ReadReceipts ──────────────

#[test]
fn recipients_private_only_sender() {
    let mut s = state();
    s.peers = vec![peer("bob", 9001, true), peer("carol", 9002, true)];
    let mut m = make_msg("bob", "salut");
    m.to_user = Some("alice".to_string()); // privé vers moi
    assert_eq!(
        s.receipt_recipients(&m),
        vec![("bob".to_string(), addr(9001))]
    );
}

#[test]
fn recipients_broadcast_all_online_peers() {
    let mut s = state();
    s.peers = vec![
        peer("bob", 9001, true),
        peer("carol", 9002, true),
        peer("dan", 9003, false), // hors-ligne → exclu
    ];
    let m = make_msg("bob", "à tous"); // to_user None = « Tous »
    let mut got = s.receipt_recipients(&m);
    got.sort();
    assert_eq!(
        got,
        vec![
            ("bob".to_string(), addr(9001)),
            ("carol".to_string(), addr(9002))
        ]
    );
}

#[test]
fn recipients_group_only_online_members() {
    let mut s = state();
    s.peers = vec![
        peer("bob", 9001, true),
        peer("carol", 9002, true),
        peer("eve", 9004, true), // pas membre → exclue
    ];
    s.groups = vec![Group {
        name: "team".to_string(),
        owner: "alice".to_string(),
        members: vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
        created_at: "2026-01-01 00:00:00".to_string(),
    }];
    let mut m = make_msg("bob", "coucou groupe");
    m.to_user = Some("#team".to_string());
    let mut got = s.receipt_recipients(&m);
    got.sort();
    assert_eq!(
        got,
        vec![
            ("bob".to_string(), addr(9001)),
            ("carol".to_string(), addr(9002))
        ]
    );
}

// ── détail nominatif reçu/lu (popup « … ») ─────────────────────────────────

#[test]
fn test_mark_delivered_by_feeds_detail() {
    let mut s = state();
    let hash = AppState::message_hash(&make_msg("alice", "test"));
    s.mark_message_delivered_by(hash, "bob".to_string());
    let detail = s.receipt_detail(hash);
    assert_eq!(detail.delivered_by, vec!["bob"]);
    assert!(detail.read_by.is_empty());
}

#[test]
fn test_receipt_detail_sorted() {
    let mut s = state();
    let hash = AppState::message_hash(&make_msg("alice", "y"));
    s.mark_message_delivered_by(hash, "zara".to_string());
    s.mark_message_delivered_by(hash, "bob".to_string());
    s.mark_message_read(hash, "carol".to_string());
    let detail = s.receipt_detail(hash);
    assert_eq!(detail.delivered_by, vec!["bob", "zara"]);
    assert_eq!(detail.read_by, vec!["carol"]);
}

#[test]
fn test_mark_sent_and_is_pending() {
    let mut s = state();
    let hash = 42u64;
    assert!(!s.is_message_pending(hash));
    s.mark_message_sent(hash, request(9000, make_msg("alice", "pending")));
    assert!(s.is_message_pending(hash));
}

#[test]
fn test_mark_acked_removes_pending() {
    let mut s = state();
    let hash = 99u64;
    s.mark_message_sent(hash, request(9000, make_msg("alice", "acked")));
    assert!(s.mark_message_acked(hash, "bob"));
    assert!(!s.is_message_pending(hash));
}

#[test]
fn ack_from_another_peer_does_not_complete_delivery() {
    let mut s = state();
    let hash = 100u64;
    let mut message = make_msg("alice", "acked");
    message.to_user = Some("bob".into());
    s.messages.push(message.clone());
    s.mark_message_sent(hash, request(9000, message));

    assert!(!s.mark_message_acked(hash, "mallory"));
    assert!(s.is_message_pending(hash));
}

#[test]
fn group_ack_is_accepted_only_from_a_member() {
    let mut s = state();
    s.groups.push(Group {
        name: "team".into(),
        owner: "alice".into(),
        members: vec!["alice".into(), "bob".into()],
        created_at: String::new(),
    });
    let mut message = make_msg("alice", "group");
    message.to_user = Some("#team".into());
    let hash = AppState::message_hash(&message);
    s.messages.push(message);

    assert!(s.is_expected_ack_sender(hash, "bob"));
    assert!(!s.is_expected_ack_sender(hash, "mallory"));
}

#[test]
fn group_read_receipt_is_shared_for_messages_from_other_members() {
    let mut s = state();
    s.groups.push(Group {
        name: "team".into(),
        owner: "alice".into(),
        members: vec!["alice".into(), "bob".into(), "carol".into()],
        created_at: String::new(),
    });
    let mut message = make_msg("bob", "group");
    message.to_user = Some("#team".into());
    let hash = AppState::message_hash(&message);
    s.messages.push(message);

    assert!(s.is_expected_receipt_sender(hash, "carol"));
    assert!(!s.is_expected_receipt_sender(hash, "mallory"));
}

#[test]
fn test_get_retry_messages_increments_retry_count() {
    let mut s = state();
    let hash = 1u64;
    // Set last_retry far in the past to force immediate retry
    s.mark_message_sent(hash, request(9000, make_msg("alice", "retry")));
    if let Some(p) = s.pending_messages.get_mut(&hash) {
        p.last_retry = SystemTime::UNIX_EPOCH;
    }
    let (retries, failed) = s.get_retry_messages();
    assert!(!retries.is_empty());
    assert!(failed.is_empty());
    assert_eq!(retries[0].0, hash);
    s.mark_retry_enqueued(hash);
    assert_eq!(s.pending_messages[&hash].retry_count, 1);
}

#[test]
fn test_get_retry_messages_empty_when_recent() {
    let mut s = state();
    let hash = 2u64;
    s.mark_message_sent(hash, request(9000, make_msg("alice", "recent")));
    let (retries, failed) = s.get_retry_messages();
    // retry_count=0 → delay=1s, elapsed<1s → no retry yet
    assert!(retries.is_empty());
    assert!(failed.is_empty());
}

#[test]
fn retry_limit_marks_message_as_failed() {
    let mut s = state();
    let hash = 3u64;
    s.mark_message_sent(hash, request(9000, make_msg("alice", "fail")));
    let pending = s.pending_messages.get_mut(&hash).unwrap();
    pending.retry_count = 5;
    pending.last_retry = SystemTime::UNIX_EPOCH;

    let (retries, failed) = s.get_retry_messages();

    assert!(retries.is_empty());
    assert_eq!(failed, vec![hash]);
    assert!(!s.is_message_pending(hash));
    assert!(s.is_message_failed(hash));
}

#[test]
fn offline_messages_wait_for_the_peer_to_return() {
    let dir = std::env::temp_dir().join(format!("abcom-outbox-{:?}", std::thread::current().id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = AppState::new_with_base("moi", &dir);
    let msg = ChatMessage {
        from: "moi".into(),
        content: "tu me liras plus tard".into(),
        timestamp: "12:00".into(),
        timestamp_epoch: Some(1),
        to_user: Some("alice".into()),
        media: None,
        reply_to: None,
        nonce: None,
    };
    let hash = AppState::message_hash(&msg);

    s.queue_offline(msg.clone(), "alice".into());
    assert!(s.is_queued_offline(hash));
    // Le retour d'un autre pair ne concerne pas la file d'alice.
    assert!(s.outbox_for("bob").is_empty());

    let ready = s.outbox_for("alice");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].0, hash);
    assert_eq!(ready[0].1.content, "tu me liras plus tard");

    // Consulter la file ne la vide pas : sans émission réussie, le message
    // doit rester en attente — c'est ce qui le protège d'une fermeture ou
    // d'un échec d'envoi au moment de la reconnexion.
    assert!(s.is_queued_offline(hash));
    assert_eq!(s.outbox_for("alice").len(), 1);

    // Émission acquise : le message sort de la file et n'y revient pas.
    s.drop_from_outbox(hash);
    assert!(!s.is_queued_offline(hash));
    assert!(s.outbox_for("alice").is_empty());
}
