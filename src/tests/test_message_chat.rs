
use super::ChatMessage;

fn msg(from: &str, content: &str, to: Option<&str>) -> ChatMessage {
    ChatMessage {
        from: from.to_string(),
        content: content.to_string(),
        timestamp: "14:00".to_string(),
        timestamp_epoch: None,
        to_user: to.map(|s| s.to_string()),
        media: None,
        reply_to: None,
    }
}

// ── Sérialisation JSON ──────────────────────────────────────────────────

#[test]
fn serialize_broadcast_omits_to_user() {
    let m = msg("alice", "bonjour", None);
    let json = serde_json::to_string(&m).unwrap();
    assert!(
        !json.contains("to_user"),
        "to_user doit être absent quand None"
    );
}

#[test]
fn serialize_private_includes_to_user() {
    let m = msg("alice", "salut", Some("bob"));
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("to_user"));
    assert!(json.contains("bob"));
}

#[test]
fn round_trip_broadcast() {
    let original = msg("alice", "hello world", None);
    let json = serde_json::to_string(&original).unwrap();
    let decoded: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.from, original.from);
    assert_eq!(decoded.content, original.content);
    assert_eq!(decoded.timestamp, original.timestamp);
    assert!(decoded.to_user.is_none());
}

#[test]
fn round_trip_private() {
    let original = msg("alice", "coucou", Some("bob"));
    let json = serde_json::to_string(&original).unwrap();
    let decoded: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.to_user, Some("bob".to_string()));
}

#[test]
fn deserialize_without_to_user_field() {
    // Ancien format sans champ to_user
    let json = r#"{"from":"alice","content":"test","timestamp":"12:00"}"#;
    let m: ChatMessage = serde_json::from_str(json).unwrap();
    assert!(m.to_user.is_none());
}

#[test]
fn deserialize_without_timestamp_epoch_defaults_to_none() {
    // Ancien format / pair non mis à jour : pas de champ timestamp_epoch
    let json = r#"{"from":"alice","content":"test","timestamp":"12:00"}"#;
    let m: ChatMessage = serde_json::from_str(json).unwrap();
    assert!(m.timestamp_epoch.is_none());
}

#[test]
fn timestamp_epoch_round_trips_and_is_omitted_when_none() {
    let with = ChatMessage {
        from: "alice".to_string(),
        content: "hi".to_string(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: Some(1_750_000_000),
        to_user: None,
        media: None,
        reply_to: None,
    };
    let json = serde_json::to_string(&with).unwrap();
    assert!(json.contains("timestamp_epoch"));
    let decoded: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.timestamp_epoch, Some(1_750_000_000));

    let without = ChatMessage {
        timestamp_epoch: None,
        ..with
    };
    let json = serde_json::to_string(&without).unwrap();
    assert!(!json.contains("timestamp_epoch"));
}

#[test]
fn reply_to_omitted_when_none() {
    let m = msg("alice", "salut", None);
    let json = serde_json::to_string(&m).unwrap();
    assert!(!json.contains("reply_to"));
}

#[test]
fn reply_to_round_trips_when_present() {
    let with = ChatMessage {
        reply_to: Some(1234),
        ..msg("alice", "salut", None)
    };
    let json = serde_json::to_string(&with).unwrap();
    assert!(json.contains("reply_to"));
    let decoded: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.reply_to, Some(1234));
}

#[test]
fn deserialize_without_reply_to_field_defaults_to_none() {
    // Ancien format / pair non mis à jour : pas de champ reply_to
    let json = r#"{"from":"alice","content":"test","timestamp":"12:00"}"#;
    let m: ChatMessage = serde_json::from_str(json).unwrap();
    assert!(m.reply_to.is_none());
}

#[test]
fn content_preserved_with_special_chars() {
    let m = msg("alice", "héllo 🎉 <script>", None);
    let json = serde_json::to_string(&m).unwrap();
    let decoded: ChatMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.content, "héllo 🎉 <script>");
}
