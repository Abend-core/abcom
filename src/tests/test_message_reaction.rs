
use super::*;

fn event(action: ReactionAction) -> ReactionEvent {
    ReactionEvent {
        message_hash: 42,
        emoji: "👍".to_string(),
        user: "alice".to_string(),
        action,
    }
}

#[test]
fn reaction_event_add_round_trips() {
    let e = event(ReactionAction::Add);
    let json = serde_json::to_string(&e).unwrap();
    let decoded: ReactionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.message_hash, 42);
    assert_eq!(decoded.emoji, "👍");
    assert_eq!(decoded.user, "alice");
    assert_eq!(decoded.action, ReactionAction::Add);
}

#[test]
fn reaction_event_remove_round_trips() {
    let e = event(ReactionAction::Remove);
    let json = serde_json::to_string(&e).unwrap();
    let decoded: ReactionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.action, ReactionAction::Remove);
}

#[test]
fn reaction_action_serializes_snake_case() {
    let json = serde_json::to_string(&ReactionAction::Add).unwrap();
    assert_eq!(json, "\"add\"");
    let json = serde_json::to_string(&ReactionAction::Remove).unwrap();
    assert_eq!(json, "\"remove\"");
}
