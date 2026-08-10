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
        nonce: None,
    }
}

#[test]
fn messages_round_trip() {
    let dir = tmp_dir("roundtrip");
    let storage = Storage::open(&dir).unwrap();
    storage
        .insert_message(&msg("alice", None, "salut", 1))
        .unwrap();
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
    let mut storage = Storage::open(&dir).unwrap();
    let entries = vec![ReactionEntry {
        emoji: "👍".to_string(),
        users: vec!["alice".to_string(), "bob".to_string()],
    }];
    storage.replace_reactions(42, &entries).unwrap();

    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    assert_eq!(loaded.reactions.get(&42).unwrap()[0].users.len(), 2);

    // Remplacement par du vide = suppression.
    storage.replace_reactions(42, &[]).unwrap();
    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    assert!(!loaded.reactions.contains_key(&42));
}

#[test]
fn read_marks_groups_and_peers_round_trip() {
    let dir = tmp_dir("tables");
    let mut storage = Storage::open(&dir).unwrap();

    storage.set_read_mark("bob", 7).unwrap();
    storage.set_read_mark("bob", 9).unwrap(); // upsert

    let group = Group {
        name: "equipe".to_string(),
        owner: "alice".to_string(),
        members: vec!["alice".to_string(), "bob".to_string()],
        created_at: "2026-07-04 10:00:00".to_string(),
    };
    storage
        .replace_groups(std::slice::from_ref(&group))
        .unwrap();

    storage.upsert_peer_alias("bob", Some("Bobby")).unwrap();
    storage.upsert_peer_avatar("bob", Some(&[1, 2, 3])).unwrap();
    storage.upsert_peer_key("bob", &[9; 32]).unwrap();

    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    assert_eq!(loaded.read_marks.get("bob"), Some(&9));
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
    storage
        .insert_message(&msg("alice", None, "public", 1))
        .unwrap();
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
    storage
        .insert_message(&msg("alice", None, "texte", 2))
        .unwrap();

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
    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "historique");
    assert_eq!(loaded.reactions.get(&hash).unwrap()[0].emoji, "❤");
    // Un ancien compteur de lecture ne se convertit pas en repère de message :
    // la source est marquée importée sans rien inventer, quitte à ce que la
    // conversation repasse « non lue » une fois.
    assert!(loaded.read_marks.is_empty());

    // Les fichiers sont retirés (renommés .bak) : pas de double import.
    assert!(!dir.join("messages.json").exists());
    assert!(dir.join("messages.json.bak").exists());

    // Simule un arrêt après commit mais avant retrait de la source : le marqueur
    // transactionnel empêche tout double import à la réouverture.
    drop(storage);
    std::fs::rename(dir.join("messages.json.bak"), dir.join("messages.json")).unwrap();
    let storage = Storage::open(&dir).unwrap();
    assert_eq!(storage.load_all(INITIAL_WINDOW).unwrap().messages.len(), 1);
    assert!(!dir.join("messages.json").exists());
    assert!(dir.join("messages.json.bak").exists());
}

#[test]
fn load_errors_are_not_hidden() {
    let dir = tmp_dir("load-errors");
    let storage = Storage::open(&dir).unwrap();
    storage
        .conn
        .execute(
            "INSERT INTO messages
             (hash, from_user, content, timestamp, media) VALUES (1, 'alice', '', '12:00', '{')",
            [],
        )
        .unwrap();

    assert!(storage.load_all(INITIAL_WINDOW).is_err());
    assert!(storage.all_media_ids().is_err());
}

#[test]
fn invalid_group_json_makes_load_all_fail() {
    let dir = tmp_dir("group-json-error");
    let storage = Storage::open(&dir).unwrap();
    storage
        .conn
        .execute("INSERT INTO groups (name, data) VALUES ('broken', '{')", [])
        .unwrap();

    assert!(storage.load_all(INITIAL_WINDOW).is_err());
}

#[test]
fn reaction_and_group_replacements_are_atomic() {
    let dir = tmp_dir("atomic-replacements");
    let mut storage = Storage::open(&dir).unwrap();
    let original_reaction = ReactionEntry {
        emoji: "ok".to_string(),
        users: vec!["alice".to_string()],
    };
    storage
        .replace_reactions(42, std::slice::from_ref(&original_reaction))
        .unwrap();
    storage
        .conn
        .execute_batch(
            "CREATE TRIGGER reject_reaction BEFORE INSERT ON reactions
             WHEN NEW.username = 'rejected'
             BEGIN SELECT RAISE(ABORT, 'rejected reaction'); END;",
        )
        .unwrap();
    let rejected_reaction = ReactionEntry {
        emoji: "bad".to_string(),
        users: vec!["rejected".to_string()],
    };
    assert!(storage.replace_reactions(42, &[rejected_reaction]).is_err());

    let original_group = Group {
        name: "original".to_string(),
        owner: "alice".to_string(),
        members: vec!["alice".to_string()],
        created_at: "2026-07-04 10:00:00".to_string(),
    };
    storage
        .replace_groups(std::slice::from_ref(&original_group))
        .unwrap();
    storage
        .conn
        .execute_batch(
            "CREATE TRIGGER reject_group BEFORE INSERT ON groups
             WHEN NEW.name = 'rejected'
             BEGIN SELECT RAISE(ABORT, 'rejected group'); END;",
        )
        .unwrap();
    let rejected_group = Group {
        name: "rejected".to_string(),
        ..original_group
    };
    assert!(storage.replace_groups(&[rejected_group]).is_err());

    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    assert_eq!(loaded.reactions[&42][0].emoji, "ok");
    assert_eq!(loaded.groups.len(), 1);
    assert_eq!(loaded.groups[0].name, "original");
}

#[test]
fn schema_migrates_nonce_and_sets_user_version() {
    let dir = tmp_dir("schema-migration");
    let conn = rusqlite::Connection::open(dir.join("abcom.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE messages (
            id INTEGER PRIMARY KEY,
            hash INTEGER NOT NULL,
            from_user TEXT NOT NULL,
            to_user TEXT,
            content TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            ts_epoch INTEGER,
            media TEXT,
            reply_to INTEGER
        );
        PRAGMA user_version = 0;",
    )
    .unwrap();
    drop(conn);

    let storage = Storage::open(&dir).unwrap();
    let version: i64 = storage
        .conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    let nonce_columns: i64 = storage
        .conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'nonce'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, super::SCHEMA_VERSION);
    assert_eq!(nonce_columns, 1);

    drop(storage);
    let storage = Storage::open(&dir).unwrap();
    let nonce_columns: i64 = storage
        .conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name = 'nonce'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(nonce_columns, 1, "la migration est idempotente");
}

#[test]
fn fresh_schema_has_current_version() {
    let dir = tmp_dir("fresh-schema");
    let storage = Storage::open(&dir).unwrap();
    let version: i64 = storage
        .conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, super::SCHEMA_VERSION);
}

#[test]
fn malformed_legacy_json_is_not_retired() {
    let dir = tmp_dir("malformed-migration");
    std::fs::write(dir.join("messages.json"), "{").unwrap();
    std::fs::write(dir.join("read_counts.json"), r#"{"bob":3}"#).unwrap();

    let storage = Storage::open(&dir).unwrap();
    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    assert!(loaded.read_marks.is_empty());
    assert!(dir.join("messages.json").exists());
    assert!(!dir.join("messages.json.bak").exists());
    assert!(!dir.join("read_counts.json").exists());
    assert!(dir.join("read_counts.json.bak").exists());
}

#[test]
fn failed_legacy_import_rolls_back_and_keeps_sources() {
    let dir = tmp_dir("migration-rollback");
    let messages = vec![msg("alice", None, "must roll back", 1)];
    std::fs::write(
        dir.join("messages.json"),
        serde_json::to_string(&messages).unwrap(),
    )
    .unwrap();
    let duplicate = Group {
        name: "duplicate".to_string(),
        owner: "alice".to_string(),
        members: vec!["alice".to_string()],
        created_at: "2026-07-04 10:00:00".to_string(),
    };
    std::fs::write(
        dir.join("groups.json"),
        serde_json::to_string(&vec![duplicate.clone(), duplicate]).unwrap(),
    )
    .unwrap();

    assert!(Storage::open(&dir).is_err());
    assert!(dir.join("messages.json").exists());
    assert!(dir.join("groups.json").exists());
    assert!(!dir.join("messages.json.bak").exists());
    assert!(!dir.join("groups.json.bak").exists());

    let conn = rusqlite::Connection::open(dir.join("abcom.db")).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
    drop(conn);

    std::fs::write(
        dir.join("groups.json"),
        serde_json::to_string(&vec![Group {
            name: "fixed".to_string(),
            owner: "alice".to_string(),
            members: vec!["alice".to_string()],
            created_at: "2026-07-04 10:00:00".to_string(),
        }])
        .unwrap(),
    )
    .unwrap();
    let reopened = Storage::open(&dir).expect("la migration doit reprendre à la réouverture");
    let loaded = reopened.load_all(INITIAL_WINDOW).unwrap();
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.groups.len(), 1);
    assert!(dir.join("messages.json.bak").exists());
    assert!(dir.join("groups.json.bak").exists());
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

#[test]
fn batched_insert_keeps_order_and_content() {
    let dir = tmp_dir("batch");
    let mut storage = Storage::open(&dir).unwrap();
    let batch: Vec<ChatMessage> = (0..5)
        .map(|i| msg("alice", None, &format!("m{i}"), i))
        .collect();
    storage.insert_messages(&batch).unwrap();
    // Un lot vide et un lot d'un seul élément empruntent les chemins courts.
    storage.insert_messages(&[]).unwrap();
    storage
        .insert_messages(&[msg("bob", None, "seul", 9)])
        .unwrap();

    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    let contents: Vec<&str> = loaded.messages.iter().map(|m| m.content.as_str()).collect();
    assert_eq!(contents, ["m0", "m1", "m2", "m3", "m4", "seul"]);
}

#[test]
fn legacy_backups_are_purged_only_once_expired() {
    let dir = tmp_dir("purge-bak");
    let fresh = dir.join("messages.json.bak");
    let dated = dir.join("messages.json.bak.1700000000");
    let unrelated = dir.join("abcom.db");
    std::fs::write(&fresh, b"[]").unwrap();
    std::fs::write(&dated, b"[]").unwrap();
    std::fs::write(&unrelated, b"").unwrap();

    // Fraîchement écrits : la purge ne doit rien toucher.
    super::purge_legacy_backups(&dir);
    assert!(fresh.exists());
    assert!(dated.exists());

    // Antidatés au-delà du délai de conservation.
    let old = std::time::SystemTime::now()
        - super::LEGACY_BACKUP_TTL
        - std::time::Duration::from_secs(60);
    for path in [&fresh, &dated] {
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(old).unwrap();
    }
    super::purge_legacy_backups(&dir);
    assert!(!fresh.exists());
    assert!(!dated.exists());
    assert!(unrelated.exists());
}

#[test]
fn receipts_survive_a_restart() {
    use super::ReceiptKind;
    let dir = tmp_dir("receipts");
    let message = msg("alice", Some("#projet"), "coucou", 1);
    let hash = AppState::message_hash(&message);
    {
        let storage = Storage::open(&dir).unwrap();
        storage.insert_message(&message).unwrap();
        storage
            .add_receipt(hash, "bob", ReceiptKind::Delivered)
            .unwrap();
        storage.add_receipt(hash, "bob", ReceiptKind::Read).unwrap();
        // Idempotent : un accusé réémis ne crée pas de doublon.
        storage.add_receipt(hash, "bob", ReceiptKind::Read).unwrap();
        storage
            .add_receipt(hash, "carol", ReceiptKind::Delivered)
            .unwrap();
    }

    let storage = Storage::open(&dir).unwrap();
    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    let delivered = loaded.delivered_receipts.get(&hash).unwrap();
    assert_eq!(delivered.len(), 2);
    assert!(delivered.contains("bob") && delivered.contains("carol"));
    assert_eq!(loaded.read_receipts.get(&hash).unwrap().len(), 1);
}

#[test]
fn orphan_receipts_are_purged_at_open() {
    use super::ReceiptKind;
    let dir = tmp_dir("receipts-orphan");
    let message = msg("alice", None, "ephemere", 1);
    let hash = AppState::message_hash(&message);
    {
        let storage = Storage::open(&dir).unwrap();
        storage.insert_message(&message).unwrap();
        storage.add_receipt(hash, "bob", ReceiptKind::Read).unwrap();
        // Le message disparaît : son accusé n'a plus de cible.
        storage.delete_conversation("me", None).unwrap();
    }

    let storage = Storage::open(&dir).unwrap();
    assert!(storage
        .load_all(INITIAL_WINDOW)
        .unwrap()
        .read_receipts
        .is_empty());
}

#[test]
fn outbox_survives_a_restart() {
    let dir = tmp_dir("outbox");
    let message = msg("moi", Some("alice"), "à la reconnexion", 1);
    let hash = AppState::message_hash(&message);
    {
        let storage = Storage::open(&dir).unwrap();
        storage.enqueue_outbox(hash, "alice", &message).unwrap();
    }

    let storage = Storage::open(&dir).unwrap();
    let loaded = storage.load_all(INITIAL_WINDOW).unwrap();
    let (peer, queued) = loaded.outbox.get(&hash).unwrap();
    assert_eq!(peer, "alice");
    assert_eq!(queued.content, "à la reconnexion");

    storage.dequeue_outbox(hash).unwrap();
    assert!(storage.load_all(INITIAL_WINDOW).unwrap().outbox.is_empty());
}

#[test]
fn export_renders_a_readable_transcript() {
    let dir = tmp_dir("export");
    let storage = Storage::open(&dir).unwrap();
    storage
        .insert_message(&msg("alice", Some("moi"), "salut", 0))
        .unwrap();
    storage
        .insert_message(&msg("moi", Some("alice"), "salut à toi", 60))
        .unwrap();
    // Une autre conversation ne doit pas fuiter dans l'export.
    storage
        .insert_message(&msg("bob", Some("moi"), "secret", 120))
        .unwrap();

    let export = storage.export_conversation("moi", Some("alice")).unwrap();
    assert!(export.contains("alice : salut"));
    assert!(export.contains("moi : salut à toi"));
    assert!(!export.contains("secret"));
    assert_eq!(export.lines().count(), 2);
}

#[test]
fn compaction_keeps_the_data_intact() {
    let dir = tmp_dir("vacuum");
    let storage = Storage::open(&dir).unwrap();
    storage
        .insert_message(&msg("alice", None, "gardé", 1))
        .unwrap();
    let (_, before) = storage.footprint(&dir).unwrap();
    storage.compact().unwrap();
    let (bytes, after) = storage.footprint(&dir).unwrap();
    assert_eq!(before, after);
    assert!(bytes > 0);
    assert_eq!(storage.load_all(INITIAL_WINDOW).unwrap().messages.len(), 1);
}

#[test]
fn schema_indexes_cover_conversation_queries() {
    let dir = tmp_dir("indexes");
    let storage = Storage::open(&dir).unwrap();
    let indexes: Vec<String> = storage
        .conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'messages'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(indexes.iter().any(|i| i == "idx_messages_conv"));

    // Le planificateur doit s'en servir plutôt que de balayer la table.
    let plan: String = storage
        .conn
        .query_row(
            "EXPLAIN QUERY PLAN SELECT id FROM messages WHERE to_user = '#projet' ORDER BY id",
            [],
            |row| row.get(3),
        )
        .unwrap();
    assert!(plan.contains("idx_messages_conv"), "plan obtenu : {plan}");
}

#[test]
fn full_text_search_finds_and_forgets() {
    let dir = tmp_dir("fts");
    let storage = Storage::open(&dir).unwrap();
    storage
        .insert_message(&msg(
            "alice",
            Some("moi"),
            "rendez-vous demain au bureau",
            1,
        ))
        .unwrap();
    storage
        .insert_message(&msg("bob", Some("moi"), "rien à voir", 2))
        .unwrap();

    let hits = storage.search("bureau", 50).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("bureau"));

    // Recherche par préfixe, au fil de la frappe.
    assert_eq!(storage.search("bur", 50).unwrap().len(), 1);
    // Plusieurs termes = ET implicite.
    assert_eq!(storage.search("rendez bureau", 50).unwrap().len(), 1);
    assert_eq!(storage.search("bureau introuvable", 50).unwrap().len(), 0);

    // Une saisie syntaxiquement hostile pour FTS5 ne doit pas faire échouer
    // la requête, juste ne rien trouver.
    assert!(storage.search("\"guillemet ouvert", 50).is_ok());
    assert!(storage.search("*", 50).is_ok());

    // Conversation effacée : l'index doit oublier, sinon il fuiterait du
    // contenu supprimé dans les résultats.
    storage.delete_conversation("moi", Some("alice")).unwrap();
    assert!(storage.search("bureau", 50).unwrap().is_empty());
}

#[test]
fn search_index_is_backfilled_for_existing_history() {
    let dir = tmp_dir("fts-backfill");
    {
        // Base créée puis index supprimé : simule une base antérieure à FTS5.
        let storage = Storage::open(&dir).unwrap();
        storage
            .insert_message(&msg("alice", None, "message historique", 1))
            .unwrap();
        storage
            .conn
            .execute_batch("DROP TRIGGER messages_fts_insert; DELETE FROM messages_fts;")
            .unwrap();
        assert!(storage.search("historique", 50).unwrap().is_empty());
    }

    let storage = Storage::open(&dir).unwrap();
    assert_eq!(storage.search("historique", 50).unwrap().len(), 1);
}
