use crate::app::{AppState, Peer};

fn state(username: &str) -> AppState {
    let mut s = AppState::new(username.to_string(), Default::default(), None);
    s.peers.clear();
    s.groups.clear();
    s.messages.clear();
    s.read_marks.clear();
    s
}

fn peer(name: &str, addr: &str, online: bool) -> Peer {
    Peer {
        username: name.to_string(),
        addr: addr.parse().unwrap(),
        last_seen: 0,
        online,
    }
}

#[test]
fn selected_transfer_targets_returns_selected_peer() {
    let mut s = state("alice");
    s.selected_conversation = Some("bob".to_string());
    s.peers.push(peer("bob", "127.0.0.1:9000", true));

    let targets = s.selected_transfer_targets();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].username, "bob");
}

#[test]
fn selected_transfer_targets_filters_group_members_to_online_peers() {
    let mut s = state("alice");
    s.peers.push(peer("bob", "127.0.0.1:9000", true));
    s.peers.push(peer("carol", "127.0.0.1:9001", false));
    let group = s
        .create_group(
            "team".to_string(),
            vec!["bob".to_string(), "carol".to_string()],
        )
        .expect("création du salon");
    s.selected_conversation = Some(AppState::group_conv_key(&group.id));

    let targets = s.selected_transfer_targets();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].username, "bob");
}
