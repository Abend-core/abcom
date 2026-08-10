use super::{apply_group_event, group_media_authorized};
use crate::app::AppState;
use crate::message::{Group, GroupAction, GroupEvent};

fn state() -> AppState {
    let mut state = AppState::new("local".to_string(), Default::default(), None);
    state.groups.clear();
    state.messages.clear();
    state
}

fn group(owner: &str, members: &[&str]) -> Group {
    Group {
        name: "Team".to_string(),
        owner: owner.to_string(),
        members: members.iter().map(|member| member.to_string()).collect(),
        created_at: String::new(),
    }
}

fn apply(state: &mut AppState, peer: &str, action: GroupAction) {
    apply_group_event(state, peer, GroupEvent { action });
}

#[test]
fn create_requires_authenticated_owner_and_local_membership() {
    let mut state = state();

    apply(
        &mut state,
        "mallory",
        GroupAction::Create {
            group: group("alice", &["alice", "local"]),
        },
    );
    apply(
        &mut state,
        "alice",
        GroupAction::Create {
            group: group("alice", &["alice"]),
        },
    );
    assert!(state.groups.is_empty());

    apply(
        &mut state,
        "alice",
        GroupAction::Create {
            group: group("alice", &["alice", "local"]),
        },
    );
    assert_eq!(state.groups.len(), 1);

    apply(
        &mut state,
        "mallory",
        GroupAction::Create {
            group: group("mallory", &["mallory", "local"]),
        },
    );
    assert_eq!(state.groups[0].owner, "alice");
}

#[test]
fn add_rename_and_delete_require_known_owner() {
    let mut state = state();
    state.groups.push(group("alice", &["alice", "local"]));

    apply(
        &mut state,
        "mallory",
        GroupAction::AddMember {
            group_name: "Team".to_string(),
            username: "bob".to_string(),
        },
    );
    apply(
        &mut state,
        "mallory",
        GroupAction::Rename {
            group_name: "Team".to_string(),
            new_name: "Hijacked".to_string(),
        },
    );
    apply(
        &mut state,
        "mallory",
        GroupAction::Delete {
            group_name: "Team".to_string(),
        },
    );
    assert_eq!(state.groups[0].name, "Team");
    assert!(!state.groups[0].members.contains(&"bob".to_string()));

    apply(
        &mut state,
        "alice",
        GroupAction::AddMember {
            group_name: "Team".to_string(),
            username: "bob".to_string(),
        },
    );
    apply(
        &mut state,
        "alice",
        GroupAction::Rename {
            group_name: "Team".to_string(),
            new_name: "Crew".to_string(),
        },
    );
    assert!(state.groups[0].members.contains(&"bob".to_string()));
    assert_eq!(state.groups[0].name, "Crew");

    apply(
        &mut state,
        "alice",
        GroupAction::Delete {
            group_name: "Crew".to_string(),
        },
    );
    assert!(state.groups.is_empty());
}

#[test]
fn remove_requires_owner_or_voluntary_departure() {
    let mut state = state();
    state
        .groups
        .push(group("alice", &["alice", "local", "bob", "charlie"]));

    apply(
        &mut state,
        "mallory",
        GroupAction::RemoveMember {
            group_name: "Team".to_string(),
            username: "bob".to_string(),
        },
    );
    assert!(state.groups[0].members.contains(&"bob".to_string()));

    apply(
        &mut state,
        "bob",
        GroupAction::RemoveMember {
            group_name: "Team".to_string(),
            username: "bob".to_string(),
        },
    );
    assert!(!state.groups[0].members.contains(&"bob".to_string()));

    apply(
        &mut state,
        "alice",
        GroupAction::RemoveMember {
            group_name: "Team".to_string(),
            username: "charlie".to_string(),
        },
    );
    assert!(!state.groups[0].members.contains(&"charlie".to_string()));
}

#[test]
fn group_media_requires_both_local_user_and_sender_membership() {
    let mut state = state();
    state
        .groups
        .push(group("alice", &["alice", "local", "bob"]));

    assert!(group_media_authorized(&state, "Team", "bob"));
    assert!(!group_media_authorized(&state, "Team", "mallory"));
    assert!(!group_media_authorized(&state, "Unknown", "bob"));
}
