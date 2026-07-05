use crate::app::AppState;
use crate::message::{ChatMessage, ReactionAction, ReactionEvent};

fn state() -> AppState {
    let mut s = AppState::new("alice".to_string(), Default::default(), None);
    s.messages.clear();
    s.peers.clear();
    s.reactions.clear();
    s
}

#[test]
fn toggle_reaction_add_when_absent() {
    let mut s = state();
    let action = s.toggle_reaction(1, "👍", "bob");
    assert_eq!(action, ReactionAction::Add);
    assert_eq!(s.reactions_for(1).len(), 1);
    assert_eq!(s.reactions_for(1)[0].users, vec!["bob".to_string()]);
}

#[test]
fn toggle_reaction_remove_when_present() {
    let mut s = state();
    s.toggle_reaction(1, "👍", "bob");
    let action = s.toggle_reaction(1, "👍", "bob");
    assert_eq!(action, ReactionAction::Remove);
    assert!(s.reactions_for(1).is_empty());
}

#[test]
fn toggle_reaction_multiple_users_same_emoji() {
    let mut s = state();
    s.toggle_reaction(1, "👍", "bob");
    s.toggle_reaction(1, "👍", "alice");
    s.toggle_reaction(1, "👍", "bob");
    let entries = s.reactions_for(1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].users, vec!["alice".to_string()]);
}

#[test]
fn apply_reaction_event_add_is_idempotent() {
    let mut s = state();
    let event = ReactionEvent {
        message_hash: 1,
        emoji: "😂".to_string(),
        user: "bob".to_string(),
        action: ReactionAction::Add,
    };
    s.apply_reaction_event(&event);
    s.apply_reaction_event(&event);
    let entries = s.reactions_for(1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].users, vec!["bob".to_string()]);
}

#[test]
fn apply_reaction_event_remove_on_absent_is_noop() {
    let mut s = state();
    let event = ReactionEvent {
        message_hash: 1,
        emoji: "😂".to_string(),
        user: "bob".to_string(),
        action: ReactionAction::Remove,
    };
    s.apply_reaction_event(&event);
    assert!(s.reactions_for(1).is_empty());
}

#[test]
fn apply_reaction_event_remove_prunes_empty_entry() {
    let mut s = state();
    s.apply_reaction_event(&ReactionEvent {
        message_hash: 1,
        emoji: "😂".to_string(),
        user: "bob".to_string(),
        action: ReactionAction::Add,
    });
    s.apply_reaction_event(&ReactionEvent {
        message_hash: 1,
        emoji: "😂".to_string(),
        user: "bob".to_string(),
        action: ReactionAction::Remove,
    });
    assert!(s.reactions_for(1).is_empty());
}

#[test]
fn reactions_for_returns_empty_slice_when_none() {
    let s = state();
    assert!(s.reactions_for(999).is_empty());
}

#[test]
fn find_message_by_hash_found() {
    let mut s = state();
    let msg = ChatMessage {
        from: "bob".to_string(),
        content: "salut".to_string(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: Some(1_750_000_000),
        to_user: None,
        media: None,
        reply_to: None,
        nonce: None,
    };
    let hash = AppState::message_hash(&msg);
    s.messages.push(msg);
    let found = s.find_message_by_hash(hash);
    assert!(found.is_some());
    assert_eq!(found.unwrap().content, "salut");
}

#[test]
fn find_message_by_hash_not_found_returns_none() {
    let s = state();
    assert!(s.find_message_by_hash(123_456).is_none());
}
