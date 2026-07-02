
use crate::app::AppState;
use crate::message::{Group, PeerRecord};
use std::path::{Path, PathBuf};

fn tmp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("abcom_test_{}_{}", label, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn state_in(dir: &Path, username: &str) -> AppState {
    AppState::new_with_base(username, dir)
}

fn make_group(name: &str, owner: &str) -> Group {
    Group {
        name: name.to_string(),
        owner: owner.to_string(),
        members: vec![owner.to_string()],
        created_at: "2026-01-01 00:00:00".to_string(),
    }
}

// ── persist_json_atomic (via save/load groups) ─────────────────────────

#[test]
fn atomic_write_creates_file() {
    let dir = tmp_dir("atomic_write");
    let mut s = state_in(&dir, "alice");
    s.groups.push(make_group("Team", "alice"));
    s.save_groups();
    assert!(dir.join("groups.json").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn atomic_write_no_tmp_file_left() {
    let dir = tmp_dir("no_tmp");
    let mut s = state_in(&dir, "alice");
    s.groups.push(make_group("Dev", "alice"));
    s.save_groups();
    // Aucun fichier .tmp ne doit subsister
    let has_tmp = std::fs::read_dir(&dir)
        .unwrap()
        .any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".tmp"));
    assert!(
        !has_tmp,
        "Fichier .tmp ne doit pas rester après écriture atomique"
    );
    std::fs::remove_dir_all(&dir).ok();
}

// ── save_groups + load_groups round-trip ───────────────────────────────

#[test]
fn groups_round_trip() {
    let dir = tmp_dir("groups_rt");
    // Sauvegarder
    let mut s1 = state_in(&dir, "alice");
    s1.groups.push(make_group("Alpha", "alice"));
    s1.groups.push(make_group("Beta", "alice"));
    s1.save_groups();

    // Charger dans un nouvel état
    let mut s2 = state_in(&dir, "alice");
    s2.load_groups();
    assert_eq!(s2.groups.len(), 2);
    assert!(s2.groups.iter().any(|g| g.name == "Alpha"));
    assert!(s2.groups.iter().any(|g| g.name == "Beta"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn groups_round_trip_empty() {
    let dir = tmp_dir("groups_empty");
    let s1 = state_in(&dir, "alice");
    s1.save_groups();

    let mut s2 = state_in(&dir, "alice");
    s2.load_groups();
    assert!(s2.groups.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

// ── save_peer_records + load_peer_records round-trip ──────────────────

#[test]
fn peer_records_round_trip() {
    let dir = tmp_dir("peer_records_rt");
    let mut s1 = state_in(&dir, "alice");
    s1.peer_records.push(PeerRecord {
        username: "bob".to_string(),
        alias: Some("Robert".to_string()),
    });
    s1.save_peer_records();

    let mut s2 = state_in(&dir, "alice");
    s2.load_peer_records();
    assert_eq!(s2.peer_records.len(), 1);
    assert_eq!(s2.peer_records[0].alias, Some("Robert".to_string()));
    std::fs::remove_dir_all(&dir).ok();
}

// ── save_messages + load_messages round-trip ──────────────────────────

#[test]
fn messages_round_trip() {
    use crate::message::ChatMessage;
    let dir = tmp_dir("messages_rt");
    let mut s1 = state_in(&dir, "alice");
    s1.messages.push(ChatMessage {
        from: "bob".to_string(),
        content: "coucou".to_string(),
        timestamp: "10:00".to_string(),
        timestamp_epoch: None,
        to_user: Some("alice".to_string()),
        media: None,
        reply_to: None,
    });
    s1.save_messages();

    let mut s2 = state_in(&dir, "alice");
    s2.load_messages();
    assert_eq!(s2.messages.len(), 1);
    assert_eq!(s2.messages[0].content, "coucou");
    std::fs::remove_dir_all(&dir).ok();
}

// ── save_reactions + load_reactions round-trip ─────────────────────────

#[test]
fn reactions_round_trip() {
    use crate::message::ReactionEntry;
    let dir = tmp_dir("reactions_rt");
    let mut s1 = state_in(&dir, "alice");
    s1.reactions.insert(
        42,
        vec![ReactionEntry {
            emoji: "👍".to_string(),
            users: vec!["bob".to_string(), "alice".to_string()],
        }],
    );
    s1.save_reactions();

    let mut s2 = state_in(&dir, "alice");
    s2.load_reactions();
    assert_eq!(s2.reactions.len(), 1);
    assert_eq!(s2.reactions[&42][0].emoji, "👍");
    assert_eq!(s2.reactions[&42][0].users.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn reactions_round_trip_empty() {
    let dir = tmp_dir("reactions_empty");
    let s1 = state_in(&dir, "alice");
    s1.save_reactions();

    let mut s2 = state_in(&dir, "alice");
    s2.load_reactions();
    assert!(s2.reactions.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

// ── load sur fichier inexistant ne panique pas ─────────────────────────

#[test]
fn load_missing_file_is_noop() {
    let dir = tmp_dir("load_missing");
    let mut s = state_in(&dir, "alice");
    // Aucun fichier sur le disque → pas de panique
    s.load_groups();
    s.load_messages();
    s.load_peer_records();
    s.load_reactions();
    assert!(s.groups.is_empty());
    assert!(s.messages.is_empty());
    assert!(s.reactions.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}
