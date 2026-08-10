use crate::app::{AppState, Peer};
use crate::message::ChatMessage;

fn new_test_state(username: &str) -> AppState {
    let mut s = AppState::new(username.to_string(), Default::default(), None);
    s.groups.clear();
    s.messages.clear();
    s.peers.clear();
    s.read_marks.clear();
    s
}

fn add_peer(s: &mut AppState, name: &str, addr: &str, online: bool) {
    s.peers.push(Peer {
        username: name.into(),
        addr: addr.parse().unwrap(),
        last_seen: 0,
        online,
    });
}

fn group_msg(from: &str, group: &str, content: &str) -> ChatMessage {
    ChatMessage {
        from: from.into(),
        content: content.into(),
        timestamp: "12:00".into(),
        timestamp_epoch: Some(0),
        to_user: Some(format!("#{group}")),
        media: None,
        reply_to: None,
        nonce: Some(ChatMessage::fresh_nonce()),
    }
}

#[test]
fn test_validate_group_name_valid() {
    assert!(AppState::validate_group_name("my-group"));
    assert!(AppState::validate_group_name("group_123"));
    assert!(AppState::validate_group_name("DevTeam"));
}

#[test]
fn test_validate_group_name_invalid() {
    assert!(!AppState::validate_group_name(""));
    assert!(!AppState::validate_group_name(&"x".repeat(51)));
    assert!(!AppState::validate_group_name("group@name"));
    assert!(!AppState::validate_group_name("group name"));
}

#[test]
fn test_create_group_success() {
    let mut s = new_test_state("alice");
    s.peers.push(Peer {
        username: "bob".into(),
        addr: "127.0.0.1:9000".parse().unwrap(),
        last_seen: 0,
        online: true,
    });
    let g = s.create_group("DevTeam".into(), vec!["bob".into()]);
    assert!(g.is_some());
    assert_eq!(s.groups[0].members.len(), 2);
}

#[test]
fn test_create_group_invalid_name() {
    let mut s = new_test_state("alice");
    assert!(s.create_group("".into(), vec![]).is_none());
}

#[test]
fn test_create_group_duplicate() {
    let mut s = new_test_state("alice");
    s.create_group("DevTeam".into(), vec![]);
    assert!(s.create_group("DevTeam".into(), vec![]).is_none());
    assert_eq!(s.groups.len(), 1);
}

#[test]
fn test_create_group_invalid_member() {
    let mut s = new_test_state("alice");
    assert!(s
        .create_group("Team".into(), vec!["unknown".into()])
        .is_none());
}

#[test]
fn test_is_group_owner() {
    let mut s = new_test_state("alice");
    s.create_group("MyGroup".into(), vec![]);
    assert!(s.is_group_owner("MyGroup"));
    assert!(!s.is_group_owner("NonExistent"));
}

#[test]
fn test_add_remove_member() {
    let mut s = new_test_state("alice");
    s.peers.push(Peer {
        username: "bob".into(),
        addr: "127.0.0.1:9000".parse().unwrap(),
        last_seen: 0,
        online: true,
    });
    s.create_group("Team".into(), vec![]);
    assert!(s.add_member_to_group("Team", "bob".into()));
    assert_eq!(s.groups[0].members.len(), 2);
    assert!(s.remove_member_from_group("Team", "bob"));
    assert_eq!(s.groups[0].members.len(), 1);
}

#[test]
fn test_group_member_addrs_online_members_only() {
    let mut s = new_test_state("alice");
    add_peer(&mut s, "bob", "192.168.1.10:9000", true);
    add_peer(&mut s, "charlie", "192.168.1.11:9000", false);
    add_peer(&mut s, "dave", "192.168.1.12:9000", true);
    // dave en ligne mais hors du groupe : jamais destinataire.
    s.create_group("Team".into(), vec!["bob".into(), "charlie".into()]);

    let addrs = s.group_member_addrs("Team");
    assert_eq!(addrs, vec!["192.168.1.10:9000".parse().unwrap()]);
    assert!(s.group_member_addrs("Inconnu").is_empty());
}

#[test]
fn test_leave_group_purges_history_and_selection() {
    let mut s = new_test_state("alice");
    add_peer(&mut s, "bob", "192.168.1.10:9000", true);
    s.create_group("Team".into(), vec!["bob".into()]);
    s.add_message(group_msg("bob", "Team", "salut"));
    s.add_message(group_msg("alice", "Team", "hello"));
    s.selected_conversation = Some("#Team".into());

    assert!(s.leave_group("Team"));
    assert!(s.groups.is_empty());
    assert!(s.messages.is_empty());
    assert_eq!(s.selected_conversation, None);
    // Groupe déjà quitté : un second départ échoue.
    assert!(!s.leave_group("Team"));
}

#[test]
fn test_member_removal_owner_succession() {
    let mut s = new_test_state("charlie");
    add_peer(&mut s, "alice", "192.168.1.9:9000", true);
    add_peer(&mut s, "bob", "192.168.1.10:9000", true);
    s.groups.push(crate::message::Group {
        name: "Team".into(),
        owner: "alice".into(),
        members: vec!["alice".into(), "bob".into(), "charlie".into()],
        created_at: String::new(),
    });

    // Départ du propriétaire : le premier membre restant hérite du groupe.
    s.apply_member_removal("Team", "alice");
    assert_eq!(s.groups[0].owner, "bob");
    assert_eq!(
        s.groups[0].members,
        vec!["bob".to_string(), "charlie".to_string()]
    );

    // Nous sommes retirés : le groupe disparaît localement.
    s.apply_member_removal("Team", "charlie");
    assert!(s.groups.is_empty());
}

#[test]
fn test_member_removal_last_member_drops_group() {
    let mut s = new_test_state("alice");
    s.groups.push(crate::message::Group {
        name: "Solo".into(),
        owner: "bob".into(),
        members: vec!["bob".into()],
        created_at: String::new(),
    });
    s.apply_member_removal("Solo", "bob");
    assert!(s.groups.is_empty());
}

#[test]
fn test_delete_group_owner_only_and_purges() {
    let mut s = new_test_state("alice");
    add_peer(&mut s, "bob", "192.168.1.10:9000", true);
    s.create_group("Team".into(), vec!["bob".into()]);
    s.add_message(group_msg("bob", "Team", "salut"));

    let mut other = new_test_state("bob");
    other.groups.push(s.groups[0].clone());
    // bob n'est pas propriétaire : suppression refusée.
    assert!(!other.delete_group("Team"));

    assert!(s.delete_group("Team"));
    assert!(s.groups.is_empty());
    assert!(s.messages.is_empty());
}

#[test]
fn test_apply_group_rename_migrates_history() {
    let mut s = new_test_state("alice");
    add_peer(&mut s, "bob", "192.168.1.10:9000", true);
    s.create_group("Team".into(), vec!["bob".into()]);
    s.add_message(group_msg("bob", "Team", "salut"));
    s.selected_conversation = Some("#Team".into());

    // Nom invalide ou doublon : refusé.
    assert!(!s.apply_group_rename("Team", "nom invalide".into()));
    s.create_group("Autre".into(), vec![]);
    assert!(!s.apply_group_rename("Team", "autre".into()));

    assert!(s.apply_group_rename("Team", "Crew".into()));
    assert!(s.get_group("Crew").is_some());
    assert_eq!(s.messages[0].to_user.as_deref(), Some("#Crew"));
    assert_eq!(s.selected_conversation.as_deref(), Some("#Crew"));
}

#[test]
fn test_group_conversation_messages_and_unread() {
    let mut s = new_test_state("alice");
    add_peer(&mut s, "bob", "192.168.1.10:9000", true);
    s.create_group("Team".into(), vec!["bob".into()]);

    s.add_message(group_msg("bob", "Team", "un"));
    s.add_message(group_msg("alice", "Team", "deux"));
    s.add_message(ChatMessage {
        to_user: None,
        ..group_msg("bob", "Team", "broadcast")
    });

    // Salon fermé : seuls les messages des autres comptent comme non-lus.
    assert_eq!(s.unread_count("#Team"), 1);

    s.selected_conversation = Some("#Team".into());
    let conv = s.get_conversation_messages();
    assert_eq!(conv.len(), 2);
    assert!(conv.iter().all(|m| m.to_user.as_deref() == Some("#Team")));

    s.mark_conversation_read("#Team");
    s.selected_conversation = None;
    assert_eq!(s.unread_count("#Team"), 0);
}

#[test]
fn test_add_member_requires_known_peer() {
    let mut s = new_test_state("alice");
    s.create_group("Team".into(), vec![]);
    assert!(!s.add_member_to_group("Team", "fantome".into()));
    add_peer(&mut s, "bob", "192.168.1.10:9000", false);
    assert!(s.add_member_to_group("Team", "bob".into()));
}
