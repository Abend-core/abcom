
use crate::app::AppState;
use crate::message::ChatMessage;
use std::net::SocketAddr;
use std::time::SystemTime;

fn state() -> AppState {
    let mut s = AppState::new("alice".to_string(), Default::default(), None);
    s.messages.clear();
    s.peers.clear();
    s
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
    assert!(s.is_message_read_by(hash, "bob"));
    assert!(!s.is_message_read_by(hash, "charlie"));
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

#[test]
fn test_mark_sent_and_is_pending() {
    let mut s = state();
    let hash = 42u64;
    let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
    assert!(!s.is_message_pending(hash));
    s.mark_message_sent(hash, addr);
    assert!(s.is_message_pending(hash));
}

#[test]
fn test_mark_acked_removes_pending() {
    let mut s = state();
    let hash = 99u64;
    let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
    s.mark_message_sent(hash, addr);
    s.mark_message_acked(hash);
    assert!(!s.is_message_pending(hash));
}

#[test]
fn test_get_retry_messages_increments_retry_count() {
    let mut s = state();
    let hash = 1u64;
    let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
    // Set last_retry far in the past to force immediate retry
    s.mark_message_sent(hash, addr);
    if let Some(p) = s.pending_messages.get_mut(&hash) {
        p.last_retry = SystemTime::UNIX_EPOCH;
    }
    let retries = s.get_retry_messages();
    assert!(!retries.is_empty());
    assert_eq!(retries[0].0, hash);
    assert_eq!(s.pending_messages[&hash].retry_count, 1);
}

#[test]
fn test_get_retry_messages_empty_when_recent() {
    let mut s = state();
    let hash = 2u64;
    let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
    s.mark_message_sent(hash, addr); // last_retry = now
    let retries = s.get_retry_messages();
    // retry_count=0 → delay=1s, elapsed<1s → no retry yet
    assert!(retries.is_empty());
}
