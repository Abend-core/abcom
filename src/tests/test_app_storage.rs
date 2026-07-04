//! Tests du moteur de stockage SQLite : aller-retour des tables, pagination
//! de l'historique et migration depuis les anciens fichiers JSON.

use std::collections::HashMap;

use super::{Storage, INITIAL_WINDOW};
use crate::app::AppState;
use crate::message::{ChatMessage, Group, ReactionEntry};

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "abcom-storage-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn msg(from: &str, to: Option<&str>, content: &str, epoch: u64) -> ChatMessage {
    ChatMessage {
        from: from.to_string(),
        content: content.to_string(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: Some(epoch),
        to_user: to.map(str::to_string),
        media: None,
        reply_to: None,
    }
}

#[test]
fn messages_round_trip() {
    let dir = tmp_dir("roundtrip");
    let storage = Storage::open(&dir).unwrap();
    storage.insert_message(&msg("alice", None, "salut", 1)).unwrap();
    storage
        .insert_message(&msg("bob", Some("alice"), "privé", 2))
        .unwrap();

    let (messages, oldest) = storage.load_recent(INITIAL_WINDOW).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "salut");
    assert_eq!(messages[1].to_user.as_deref(), Some("alice"));
    // Tout tient dans la fenêtre : rien à paginer.
    assert!(oldest.is_none());
}

#[test]
fn load_recent_windows_and_load_older_paginates() {
    let dir = tmp_dir("pagination");
    let storage = Storage::open(&dir).unwrap();
    for i in 0..10 {
        storage
            .insert_message(&msg("alice", None, &format!("m{i}"), i))
            .unwrap();
    }

    // Fenêtre de 4 : les 4 derniers, avec un point de pagination.
    let (recent, oldest) = storage.load_recent(4).unwrap();
    assert_eq!(recent.len(), 4);
    assert_eq!(recent[0].content, "m6");
    assert_eq!(recent[3].content, "m9");
    let oldest = oldest.expect("il reste de l'historique");

    // Page précédente de 4.
    let (older, older_prev) = storage.load_older(oldest, 4).unwrap();
    assert_eq!(older.len(), 4);
    assert_eq!(older[0].content, "m2");
    assert_eq!(older[3].content, "m5");
    let older_prev = older_prev.expect("encore deux messages");

    // Dernière page : plus rien au-delà.
    let (first, none) = storage.load_older(older_prev, 4).unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].content, "m0");
    assert!(none.is_none());
}

#[test]
fn reactions_round_trip_and_clear() {
    let dir = tmp_dir("reactions");
    let storage = Storage::open(&dir).unwrap();
    let entries = vec![ReactionEntry {
        emoji: "👍".to_string(),
        users: vec!["alice".to_string(), "bob".to_string()],
    }];
    storage.replace_reactions(42, &entries).unwrap();

    let loaded = storage.load_all(INITIAL_WINDOW);
    assert_eq!(loaded.reactions.get(&42).unwrap()[0].users.len(), 2);

    // Remplacement par du vide = suppression.
    storage.replace_reactions(42, &[]).unwrap();
    let loaded = storage.load_all(INITIAL_WINDOW);
    assert!(loaded.reactions.get(&42).is_none());
}

#[test]
fn read_counts_groups_and_peers_round_trip() {
    let dir = tmp_dir("tables");
    let storage = Storage::open(&dir).unwrap();

    storage.set_read_count("bob", 7).unwrap();
    storage.set_read_count("bob", 9).unwrap(); // upsert

    let group = Group {
        name: "equipe".to_string(),
        owner: "alice".to_string(),
        members: vec!["alice".to_string(), "bob".to_string()],
        created_at: "2026-07-04 10:00:00".to_string(),
    };
    storage.replace_groups(std::slice::from_ref(&group)).unwrap();

    storage.upsert_peer_alias("bob", Some("Bobby")).unwrap();
    storage.upsert_peer_avatar("bob", Some(&[1, 2, 3])).unwrap();
    storage.upsert_peer_key("bob", &[9; 32]).unwrap();

    let loaded = storage.load_all(INITIAL_WINDOW);
    assert_eq!(loaded.read_counts.get("bob"), Some(&9));
    assert_eq!(loaded.groups.len(), 1);
    assert_eq!(loaded.groups[0].owner, "alice");
    assert_eq!(loaded.peer_records[0].alias.as_deref(), Some("Bobby"));
    assert_eq!(loaded.peer_avatars.get("bob").unwrap(), &vec![1, 2, 3]);
    assert_eq!(loaded.peer_keys.get("bob").unwrap(), &vec![9u8; 32]);
}

#[test]
fn delete_conversation_broadcast_and_private() {
    let dir = tmp_dir("delete");
    let storage = Storage::open(&dir).unwrap();
    storage.insert_message(&msg("alice", None, "public", 1)).unwrap();
    storage
        .insert_message(&msg("bob", Some("me"), "vers moi", 2))
        .unwrap();
    storage
        .insert_message(&msg("me", Some("bob"), "vers bob", 3))
        .unwrap();

    storage.delete_conversation("me", None).unwrap();
    let (messages, _) = storage.load_recent(INITIAL_WINDOW).unwrap();
    assert_eq!(messages.len(), 2, "le broadcast est parti");

    storage.delete_conversation("me", Some("bob")).unwrap();
    let (messages, _) = storage.load_recent(INITIAL_WINDOW).unwrap();
    assert!(messages.is_empty(), "la conversation privée est partie");
}

#[test]
fn media_ids_and_delete_by_media_id() {
    let dir = tmp_dir("media");
    let storage = Storage::open(&dir).unwrap();
    let mut with_media = msg("alice", None, "", 1);
    with_media.media = Some(crate::message::MediaAttachment {
        id: "123-photo.png".to_string(),
        filename: "photo.png".to_string(),
        kind: crate::message::MediaKind::Image,
        size_bytes: 42,
        width: None,
        height: None,
        url: None,
    });
    storage.insert_message(&with_media).unwrap();
    storage.insert_message(&msg("alice", None, "texte", 2)).unwrap();

    let ids = storage.all_media_ids().unwrap();
    assert!(ids.contains("123-photo.png"));

    storage.delete_by_media_id("123-photo.png").unwrap();
    let (messages, _) = storage.load_recent(INITIAL_WINDOW).unwrap();
    assert_eq!(messages.len(), 1);
    assert!(storage.all_media_ids().unwrap().is_empty());
}

#[test]
fn migrates_legacy_json_once() {
    let dir = tmp_dir("migration");
    // Anciens fichiers JSON de la version pré-SQLite.
    let messages = vec![msg("alice", None, "historique", 1)];
    std::fs::write(
        dir.join("messages.json"),
        serde_json::to_string(&messages).unwrap(),
    )
    .unwrap();
    let mut reactions: HashMap<u64, Vec<ReactionEntry>> = HashMap::new();
    let hash = AppState::message_hash(&messages[0]);
    reactions.insert(
        hash,
        vec![ReactionEntry {
            emoji: "❤".to_string(),
            users: vec!["bob".to_string()],
        }],
    );
    std::fs::write(
        dir.join("reactions.json"),
        serde_json::to_string(&reactions).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.join("read_counts.json"), r#"{"bob":3}"#).unwrap();

    let storage = Storage::open(&dir).unwrap();
    let loaded = storage.load_all(INITIAL_WINDOW);
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "historique");
    assert_eq!(loaded.reactions.get(&hash).unwrap()[0].emoji, "❤");
    assert_eq!(loaded.read_counts.get("bob"), Some(&3));

    // Les fichiers sont retirés (renommés .bak) : pas de double import.
    assert!(!dir.join("messages.json").exists());
    assert!(dir.join("messages.json.bak").exists());

    // Réouverture : la base existe, pas de re-migration.
    drop(storage);
    let storage = Storage::open(&dir).unwrap();
    assert_eq!(storage.load_all(INITIAL_WINDOW).messages.len(), 1);
}

#[test]
fn prepend_older_extends_window() {
    let mut s = AppState::new_with_base("me", &tmp_dir("prepend"));
    s.messages = vec![msg("alice", None, "recent", 10)];
    let before_generation = s.content_generation;

    s.prepend_older_messages(vec![msg("alice", None, "ancien", 1)], None);
    assert_eq!(s.messages.len(), 2);
    assert_eq!(s.messages[0].content, "ancien");
    assert!(s.oldest_loaded_rowid.is_none());
    assert_ne!(s.content_generation, before_generation);

    // Une page vide n'invalide rien.
    let generation = s.content_generation;
    s.prepend_older_messages(Vec::new(), None);
    assert_eq!(s.content_generation, generation);
}
