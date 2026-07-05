use crate::app::AppState;

fn tmp_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("abcom_avatar_{}_{}", label, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn my_avatar_round_trip() {
    let dir = tmp_dir("mine");
    let mut s1 = AppState::new_with_base("alice", &dir);
    s1.set_my_avatar(vec![9, 8, 7]);
    assert_eq!(s1.avatar_bytes("alice"), Some(vec![9, 8, 7]));

    let mut s2 = AppState::new_with_base("alice", &dir);
    s2.load_avatar();
    assert_eq!(s2.my_avatar, Some(vec![9, 8, 7]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn clear_my_avatar_removes_file() {
    let dir = tmp_dir("clear");
    let mut s = AppState::new_with_base("alice", &dir);
    s.set_my_avatar(vec![1, 2, 3]);
    s.clear_my_avatar();
    assert!(s.my_avatar.is_none());
    assert!(!dir.join("avatar.png").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn peer_avatar_set_and_remove() {
    let dir = tmp_dir("peer");
    let mut s = AppState::new_with_base("alice", &dir);
    s.set_peer_avatar("bob".to_string(), vec![4, 5]);
    assert_eq!(s.avatar_bytes("bob"), Some(vec![4, 5]));
    s.set_peer_avatar("bob".to_string(), Vec::new());
    assert!(s.avatar_bytes("bob").is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn avatar_announce_built_from_own_avatar() {
    let dir = tmp_dir("announce");
    let mut s = AppState::new_with_base("alice", &dir);
    assert!(s.avatar_announce().is_none());
    s.set_my_avatar(vec![1]);
    let announce = s.avatar_announce().unwrap();
    assert_eq!(announce.from, "alice");
    assert_eq!(announce.png, vec![1]);
    std::fs::remove_dir_all(&dir).ok();
}
