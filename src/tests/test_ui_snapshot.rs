use super::SidebarCache;
use crate::app::AppState;
use std::net::SocketAddr;

fn state() -> AppState {
    let dir = std::env::temp_dir().join(format!(
        "abcom_sidebar_cache_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    AppState::new_with_base("alice", &dir)
}

fn addr(ip: &str) -> SocketAddr {
    format!("{}:9000", ip).parse().unwrap()
}

#[test]
fn pinned_peer_sorts_before_unpinned() {
    let mut s = state();
    s.add_peer("bob".to_string(), addr("192.168.1.5"));
    s.add_peer("carol".to_string(), addr("192.168.1.6"));
    s.toggle_pinned("carol");

    let mut cache = SidebarCache::default();
    cache.refresh(&s);

    assert_eq!(cache.peers[0].username, "carol");
    assert_eq!(cache.peers[1].username, "bob");
    assert!(cache.peer_pinned[0]);
    assert!(!cache.peer_pinned[1]);
}

#[test]
fn pinned_group_sorts_before_unpinned() {
    let mut s = state();
    s.create_group("alpha".to_string(), vec![]);
    s.create_group("beta".to_string(), vec![]);
    s.toggle_pinned(&AppState::group_conv_key("beta"));

    let mut cache = SidebarCache::default();
    cache.refresh(&s);

    assert_eq!(cache.groups[0].name, "beta");
    assert_eq!(cache.groups[1].name, "alpha");
    assert!(cache.group_pinned[0]);
    assert!(!cache.group_pinned[1]);
}
