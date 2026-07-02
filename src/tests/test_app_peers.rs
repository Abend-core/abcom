
use crate::app::{AppState, Peer};
use std::net::SocketAddr;

fn state(username: &str) -> AppState {
    let mut s = AppState::new(username.to_string());
    s.peers.clear();
    s.messages.clear();
    s.groups.clear();
    s.read_counts.clear();
    s.peer_records.clear();
    s
}

fn peer(name: &str, ip: &str, online: bool) -> Peer {
    let addr: SocketAddr = format!("{}:9000", ip).parse().unwrap();
    Peer {
        username: name.to_string(),
        addr,
        last_seen: 0,
        online,
    }
}

#[test]
fn test_add_peer_new() {
    let mut s = state("alice");
    // L'adresse fournie (IP + port de chat annoncé) est stockée telle quelle
    let addr: SocketAddr = "192.168.1.5:9010".parse().unwrap();
    s.add_peer("bob".to_string(), addr);
    assert_eq!(s.peers.len(), 1);
    assert_eq!(s.peers[0].username, "bob");
    assert_eq!(s.peers[0].addr, addr);
    assert!(s.peers[0].online);
}

#[test]
fn test_add_peer_updates_existing() {
    let mut s = state("alice");
    let a1: SocketAddr = "192.168.1.5:1234".parse().unwrap();
    let a2: SocketAddr = "192.168.1.6:1234".parse().unwrap();
    s.add_peer("bob".to_string(), a1);
    s.add_peer("bob".to_string(), a2);
    assert_eq!(s.peers.len(), 1, "no duplicate");
    assert_eq!(s.peers[0].addr.ip().to_string(), "192.168.1.6");
}

#[test]
fn test_cleanup_inactive_peers() {
    let mut s = state("alice");
    // last_seen = 0 → very old, online = true
    s.peers.push(peer("bob", "192.168.1.5", true));
    let disc = s.cleanup_inactive_peers(1);
    assert_eq!(disc, vec!["bob".to_string()]);
    assert!(!s.peers[0].online);
}

#[test]
fn test_cleanup_inactive_peers_recent_stays_online() {
    let mut s = state("alice");
    // last_seen very recent: use std epoch + current time
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    s.peers.push(Peer {
        username: "bob".to_string(),
        addr: "192.168.1.5:9000".parse().unwrap(),
        last_seen: now,
        online: true,
    });
    let disc = s.cleanup_inactive_peers(30);
    assert!(disc.is_empty());
    assert!(s.peers[0].online);
}

#[test]
fn test_is_peer_online() {
    let mut s = state("alice");
    s.peers.push(peer("bob", "192.168.1.5", true));
    s.peers.push(peer("charlie", "192.168.1.6", false));
    assert!(s.is_peer_online("bob"));
    assert!(!s.is_peer_online("charlie"));
    assert!(!s.is_peer_online("nobody"));
}

#[test]
fn test_get_online_peers() {
    let mut s = state("alice");
    s.peers.push(peer("bob", "192.168.1.5", true));
    s.peers.push(peer("charlie", "192.168.1.6", false));
    let online = s.get_online_peers();
    assert_eq!(online.len(), 1);
    assert_eq!(online[0].ip().to_string(), "192.168.1.5");
}

#[test]
fn test_selected_peer_addr_none_when_no_selection() {
    let s = state("alice");
    assert!(s.selected_peer_addr().is_none());
}

#[test]
fn test_selected_peer_addr_returns_addr() {
    let mut s = state("alice");
    s.peers.push(peer("bob", "192.168.1.5", true));
    s.selected_conversation = Some("bob".to_string());
    assert!(s.selected_peer_addr().is_some());
    assert_eq!(
        s.selected_peer_addr().unwrap().ip().to_string(),
        "192.168.1.5"
    );
}

#[test]
fn test_selected_peer_addr_none_when_offline() {
    let mut s = state("alice");
    s.peers.push(peer("bob", "192.168.1.5", false));
    s.selected_conversation = Some("bob".to_string());
    assert!(s.selected_peer_addr().is_none());
}

#[test]
fn test_peer_display_name_no_alias() {
    let s = state("alice");
    assert_eq!(s.peer_display_name("bob"), "bob");
}

#[test]
fn test_peer_display_name_with_alias() {
    use crate::message::PeerRecord;
    let mut s = state("alice");
    s.peer_records.push(PeerRecord {
        username: "bob".to_string(),
        alias: Some("Robert".to_string()),
    });
    assert_eq!(s.peer_display_name("bob"), "Robert");
}

#[test]
fn test_set_peer_alias_then_clear() {
    // new_with_base isole les écritures disque dans un répertoire temporaire
    let dir = std::env::temp_dir().join(format!("abcom_alias_{}", std::process::id()));
    let mut s = AppState::new_with_base("alice", &dir);
    s.set_peer_alias("bob", Some("Robert".to_string()));
    assert_eq!(s.peer_display_name("bob"), "Robert");
    s.set_peer_alias("bob", None);
    assert_eq!(s.peer_display_name("bob"), "bob");
    let _ = std::fs::remove_dir_all(&dir);
}
