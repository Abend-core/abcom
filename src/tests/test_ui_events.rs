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
        id: "Team".to_string(),
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
            group_id: "Team".to_string(),
            username: "bob".to_string(),
        },
    );
    apply(
        &mut state,
        "mallory",
        GroupAction::Rename {
            group_id: "Team".to_string(),
            new_name: "Hijacked".to_string(),
        },
    );
    apply(
        &mut state,
        "mallory",
        GroupAction::Delete {
            group_id: "Team".to_string(),
        },
    );
    assert_eq!(state.groups[0].name, "Team");
    assert!(!state.groups[0].members.contains(&"bob".to_string()));

    apply(
        &mut state,
        "alice",
        GroupAction::AddMember {
            group_id: "Team".to_string(),
            username: "bob".to_string(),
        },
    );
    apply(
        &mut state,
        "alice",
        GroupAction::Rename {
            group_id: "Team".to_string(),
            new_name: "Crew".to_string(),
        },
    );
    assert!(state.groups[0].members.contains(&"bob".to_string()));
    assert_eq!(state.groups[0].name, "Crew");
    // Le renommage n'a pas touché à l'identité du salon : les événements
    // suivants le désignent toujours par le même identifiant.
    assert_eq!(state.groups[0].id, "Team");

    apply(
        &mut state,
        "alice",
        GroupAction::Delete {
            group_id: "Team".to_string(),
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
            group_id: "Team".to_string(),
            username: "bob".to_string(),
        },
    );
    assert!(state.groups[0].members.contains(&"bob".to_string()));

    apply(
        &mut state,
        "bob",
        GroupAction::RemoveMember {
            group_id: "Team".to_string(),
            username: "bob".to_string(),
        },
    );
    assert!(!state.groups[0].members.contains(&"bob".to_string()));

    apply(
        &mut state,
        "alice",
        GroupAction::RemoveMember {
            group_id: "Team".to_string(),
            username: "charlie".to_string(),
        },
    );
    assert!(!state.groups[0].members.contains(&"charlie".to_string()));
}

/// Une réception de média n'émettait aucun accusé de livraison : l'en-tête ne
/// passe pas par `MessageReceived`, qui acquitte les messages texte. Un fichier
/// restait donc éternellement « non reçu » chez l'émetteur, tout en pouvant
/// être marqué « lu » — l'incohérence constatée dans le fil « Tous ».
#[test]
fn a_finished_media_reception_acknowledges_delivery() {
    use crate::message::{MediaAttachment, MediaKind, NetworkPacket};

    let mut state = state();
    state.add_peer("alice".to_string(), "127.0.0.1:9000".parse().unwrap());
    let mut message = crate::message::ChatMessage {
        from: "alice".to_string(),
        content: String::new(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: Some(1),
        to_user: None,
        media: None,
        reply_to: None,
        nonce: None,
    };
    message.media = Some(MediaAttachment {
        id: "2026-08-12_120000-000001-rapport.pdf".to_string(),
        filename: "rapport.pdf".to_string(),
        kind: MediaKind::File,
        size_bytes: 10,
        url: None,
        width: None,
        height: None,
    });
    let hash = AppState::message_hash(&message);
    state.add_message(message);

    let acks = super::delivery_acks(&state, "2026-08-12_120000-000001-rapport.pdf");

    assert_eq!(acks.len(), 1, "un accusé pour l'émetteur : {acks:?}");
    assert_eq!(acks[0].to_peer, "alice");
    match &acks[0].packet {
        NetworkPacket::Ack(ack) => {
            assert_eq!(ack.message_hash, hash);
            assert_eq!(ack.from, "local");
        }
        other => panic!("un accusé de livraison était attendu : {other:?}"),
    }
}

/// On n'acquitte jamais son propre envoi : la progression « terminé » se
/// déclenche aussi côté émetteur.
#[test]
fn our_own_media_is_never_self_acknowledged() {
    use crate::message::{MediaAttachment, MediaKind};

    let mut state = state();
    state.add_peer("alice".to_string(), "127.0.0.1:9000".parse().unwrap());
    let mut message = crate::message::ChatMessage {
        from: "local".to_string(),
        content: String::new(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: Some(1),
        to_user: None,
        media: None,
        reply_to: None,
        nonce: None,
    };
    message.media = Some(MediaAttachment {
        id: "envoi.pdf".to_string(),
        filename: "envoi.pdf".to_string(),
        kind: MediaKind::File,
        size_bytes: 10,
        url: None,
        width: None,
        height: None,
    });
    state.add_message(message);

    assert!(super::delivery_acks(&state, "envoi.pdf").is_empty());
    // Média inconnu : aucun accusé, aucune panique.
    assert!(super::delivery_acks(&state, "jamais-vu.pdf").is_empty());
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
