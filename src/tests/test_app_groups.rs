use crate::app::{AppState, Peer};

fn new_test_state(username: &str) -> AppState {
    let mut s = AppState::new(username.to_string(), Default::default(), None);
    s.groups.clear();
    s.messages.clear();
    s.peers.clear();
    s.read_counts.clear();
    s
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
fn test_get_online_peers() {
    let mut s = new_test_state("alice");
    s.peers.push(Peer {
        username: "bob".into(),
        addr: "192.168.1.10:9000".parse().unwrap(),
        last_seen: 0,
        online: true,
    });
    s.peers.push(Peer {
        username: "charlie".into(),
        addr: "192.168.1.11:9000".parse().unwrap(),
        last_seen: 0,
        online: false,
    });
    let online = s.get_online_peers();
    assert_eq!(online.len(), 1);
    assert!(online.contains(&"192.168.1.10:9000".parse().unwrap()));
}
