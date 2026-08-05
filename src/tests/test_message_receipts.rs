use super::{MessageAck, ReadReceipt, TypingIndicator};

// ── TypingIndicator ─────────────────────────────────────────────────────

#[test]
fn typing_indicator_round_trip() {
    let t = TypingIndicator {
        from: "alice".to_string(),
    };
    let json = serde_json::to_string(&t).unwrap();
    let decoded: TypingIndicator = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.from, "alice");
}

// ── ReadReceipt ─────────────────────────────────────────────────────────

#[test]
fn read_receipt_round_trip() {
    let r = ReadReceipt {
        from: "bob".to_string(),
        to: "alice".to_string(),
        message_hash: 123456789,
        timestamp: "14:00".to_string(),
    };
    let json = serde_json::to_string(&r).unwrap();
    let decoded: ReadReceipt = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.from, "bob");
    assert_eq!(decoded.to, "alice");
    assert_eq!(decoded.message_hash, 123456789);
    assert_eq!(decoded.timestamp, "14:00");
}

#[test]
fn read_receipt_hash_preserves_zero() {
    let r = ReadReceipt {
        from: "a".to_string(),
        to: "b".to_string(),
        message_hash: 0,
        timestamp: "".to_string(),
    };
    let json = serde_json::to_string(&r).unwrap();
    let decoded: ReadReceipt = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.message_hash, 0);
}

// ── MessageAck ──────────────────────────────────────────────────────────

#[test]
fn message_ack_round_trip() {
    let a = MessageAck {
        from: "alice".to_string(),
        to: "bob".to_string(),
        message_hash: 987654321,
        timestamp: "15:30".to_string(),
    };
    let json = serde_json::to_string(&a).unwrap();
    let decoded: MessageAck = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.from, "alice");
    assert_eq!(decoded.message_hash, 987654321);
}
