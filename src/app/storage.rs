//! Persistance SQLite (remplace les fichiers JSON).
//!
//! Toute l'I/O passe par un **thread de stockage dédié** : les mutations de
//! [`AppState`](super::AppState) envoient des [`StorageCmd`] (O(1), aucune
//! sérialisation ni écriture disque dans le thread UI). L'historique complet
//! vit en base ; la mémoire ne charge qu'une fenêtre récente, étendue à la
//! demande par [`StorageCmd::LoadOlder`] (pagination façon Discord).
//!
//! Migration : au premier lancement avec une base absente, les anciens
//! fichiers JSON (`messages.json`, `reactions.json`, …) sont importés puis
//! renommés en `.bak`.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, SyncSender};

use rusqlite::{params, Connection};

use crate::message::{AppEvent, ChatMessage, Group, PeerRecord, ReactionEntry};

/// Fenêtre de messages chargée en mémoire au démarrage.
pub const INITIAL_WINDOW: u32 = 500;
/// Taille d'une page de chargement d'historique (scroll vers le haut).
pub const OLDER_PAGE: u32 = 100;

/// Commandes du thread de stockage (FIFO : l'ordre des mutations est
/// préservé ; `Flush` répond une fois toutes les commandes précédentes
/// appliquées).
pub enum StorageCmd {
    InsertMessage(ChatMessage),
    /// Efface une conversation : `None` = fil « Tous » (broadcast).
    DeleteConversation {
        me: String,
        conv: Option<String>,
    },
    DeleteMessageByMediaId(String),
    /// Remplace l'ensemble des réactions d'un message (vide = suppression).
    ReplaceReactions {
        hash: u64,
        entries: Vec<ReactionEntry>,
    },
    SetReadCount {
        username: String,
        count: u64,
    },
    ReplaceGroups(Vec<Group>),
    UpsertPeerAlias {
        username: String,
        alias: Option<String>,
    },
    UpsertPeerAvatar {
        username: String,
        avatar: Option<Vec<u8>>,
    },
    /// Clé publique épinglée d'un pair (TOFU, transport chiffré).
    UpsertPeerKey {
        username: String,
        pubkey: Vec<u8>,
    },
    /// Charge la page précédente de l'historique ; le résultat revient à
    /// l'UI via `AppEvent::OlderMessagesLoaded`.
    LoadOlder {
        before_rowid: i64,
    },
    /// Préférence persistée (table kv) : notifications, autostart…
    SetKv {
        k: String,
        v: String,
    },
    /// Accusé de traitement : toutes les commandes précédentes sont écrites.
    Flush(SyncSender<()>),
}

/// État initial chargé depuis la base au démarrage.
#[derive(Default)]
pub struct LoadedState {
    pub messages: Vec<ChatMessage>,
    /// rowid du plus ancien message chargé (`None` = historique entier en
    /// mémoire, plus rien à paginer).
    pub oldest_rowid: Option<i64>,
    pub reactions: HashMap<u64, Vec<ReactionEntry>>,
    pub read_counts: HashMap<String, usize>,
    pub groups: Vec<Group>,
    pub peer_records: Vec<PeerRecord>,
    pub peer_avatars: HashMap<String, Vec<u8>>,
    pub peer_keys: HashMap<String, Vec<u8>>,
    /// Préférences persistées (clé → valeur).
    pub kv: HashMap<String, String>,
}

pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Ouvre (ou crée) la base, applique le schéma et importe les anciens
    /// fichiers JSON si la base vient d'être créée.
    pub fn open(base: &Path) -> rusqlite::Result<Self> {
        std::fs::create_dir_all(base).ok();
        let db_path = base.join("abcom.db");
        let fresh = !db_path.exists();
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id        INTEGER PRIMARY KEY,
                hash      INTEGER NOT NULL,
                from_user TEXT    NOT NULL,
                to_user   TEXT,
                content   TEXT    NOT NULL,
                timestamp TEXT    NOT NULL,
                ts_epoch  INTEGER,
                media     TEXT,
                reply_to  INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_messages_hash ON messages(hash);
            CREATE TABLE IF NOT EXISTS reactions (
                message_hash INTEGER NOT NULL,
                emoji        TEXT    NOT NULL,
                username     TEXT    NOT NULL,
                PRIMARY KEY (message_hash, emoji, username)
            );
            CREATE TABLE IF NOT EXISTS read_counts (
                username TEXT PRIMARY KEY,
                count    INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS groups (
                name TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS peers (
                username TEXT PRIMARY KEY,
                alias    TEXT,
                avatar   BLOB,
                pubkey   BLOB
            );
            CREATE TABLE IF NOT EXISTS kv (k TEXT PRIMARY KEY, v BLOB);",
        )?;
        // Colonne ajoutée après coup (bases existantes) : l'erreur « duplicate
        // column » est attendue et ignorée.
        let _ = conn.execute("ALTER TABLE messages ADD COLUMN nonce INTEGER", []);
        let mut storage = Self { conn };
        if fresh {
            storage.migrate_from_json(base);
        }
        Ok(storage)
    }

    // ── Écritures ────────────────────────────────────────────────────────

    pub fn insert_message(&self, msg: &ChatMessage) -> rusqlite::Result<()> {
        let media = msg
            .media
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok());
        self.conn.execute(
            "INSERT INTO messages (hash, from_user, to_user, content, timestamp, ts_epoch, media, reply_to, nonce)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                super::AppState::message_hash(msg) as i64,
                msg.from,
                msg.to_user,
                msg.content,
                msg.timestamp,
                msg.timestamp_epoch.map(|e| e as i64),
                media,
                msg.reply_to.map(|h| h as i64),
                msg.nonce.map(|n| n as i64),
            ],
        )?;
        Ok(())
    }

    pub fn delete_conversation(&self, me: &str, conv: Option<&str>) -> rusqlite::Result<()> {
        match conv {
            None => {
                self.conn
                    .execute("DELETE FROM messages WHERE to_user IS NULL", [])?;
            }
            Some(user) => {
                self.conn.execute(
                    "DELETE FROM messages
                     WHERE (from_user = ?1 AND to_user = ?2)
                        OR (from_user = ?2 AND to_user = ?1)",
                    params![user, me],
                )?;
            }
        }
        Ok(())
    }

    pub fn delete_by_media_id(&self, media_id: &str) -> rusqlite::Result<()> {
        // Le média est stocké en JSON : filtre sur l'id exact.
        self.conn.execute(
            "DELETE FROM messages
             WHERE media IS NOT NULL AND json_extract(media, '$.id') = ?1",
            params![media_id],
        )?;
        Ok(())
    }

    pub fn replace_reactions(&self, hash: u64, entries: &[ReactionEntry]) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM reactions WHERE message_hash = ?1",
            params![hash as i64],
        )?;
        let mut stmt = self.conn.prepare_cached(
            "INSERT OR IGNORE INTO reactions (message_hash, emoji, username) VALUES (?1, ?2, ?3)",
        )?;
        for entry in entries {
            for user in &entry.users {
                stmt.execute(params![hash as i64, entry.emoji, user])?;
            }
        }
        Ok(())
    }

    pub fn set_read_count(&self, username: &str, count: u64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO read_counts (username, count) VALUES (?1, ?2)
             ON CONFLICT(username) DO UPDATE SET count = excluded.count",
            params![username, count as i64],
        )?;
        Ok(())
    }

    pub fn replace_groups(&self, groups: &[Group]) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM groups", [])?;
        let mut stmt = self
            .conn
            .prepare_cached("INSERT INTO groups (name, data) VALUES (?1, ?2)")?;
        for group in groups {
            if let Ok(data) = serde_json::to_string(group) {
                stmt.execute(params![group.name, data])?;
            }
        }
        Ok(())
    }

    pub fn upsert_peer_alias(&self, username: &str, alias: Option<&str>) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO peers (username, alias) VALUES (?1, ?2)
             ON CONFLICT(username) DO UPDATE SET alias = excluded.alias",
            params![username, alias],
        )?;
        Ok(())
    }

    pub fn upsert_peer_avatar(
        &self,
        username: &str,
        avatar: Option<&[u8]>,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO peers (username, avatar) VALUES (?1, ?2)
             ON CONFLICT(username) DO UPDATE SET avatar = excluded.avatar",
            params![username, avatar],
        )?;
        Ok(())
    }

    pub fn set_kv(&self, k: &str, v: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO kv (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![k, v.as_bytes()],
        )?;
        Ok(())
    }

    pub fn upsert_peer_key(&self, username: &str, pubkey: &[u8]) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO peers (username, pubkey) VALUES (?1, ?2)
             ON CONFLICT(username) DO UPDATE SET pubkey = excluded.pubkey",
            params![username, pubkey],
        )?;
        Ok(())
    }

    // ── Lectures ─────────────────────────────────────────────────────────

    fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, ChatMessage)> {
        let rowid: i64 = row.get(0)?;
        let media: Option<String> = row.get(6)?;
        Ok((
            rowid,
            ChatMessage {
                from: row.get(1)?,
                to_user: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get(4)?,
                timestamp_epoch: row.get::<_, Option<i64>>(5)?.map(|e| e as u64),
                media: media.and_then(|m| serde_json::from_str(&m).ok()),
                reply_to: row.get::<_, Option<i64>>(7)?.map(|h| h as u64),
                nonce: row.get::<_, Option<i64>>(8)?.map(|n| n as u64),
            },
        ))
    }

    const MSG_COLS: &'static str =
        "id, from_user, to_user, content, timestamp, ts_epoch, media, reply_to, nonce";

    /// Derniers `limit` messages (ordre chronologique) + rowid du plus ancien
    /// chargé (None si toute la base tient dans la fenêtre).
    pub fn load_recent(&self, limit: u32) -> rusqlite::Result<(Vec<ChatMessage>, Option<i64>)> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {} FROM messages ORDER BY id DESC LIMIT ?1",
            Self::MSG_COLS
        ))?;
        let mut rows: Vec<(i64, ChatMessage)> = stmt
            .query_map(params![limit], Self::row_to_message)?
            .collect::<Result<_, _>>()?;
        rows.reverse();
        let oldest = rows.first().map(|(id, _)| *id);
        let remaining: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE id < ?1",
            params![oldest.unwrap_or(0)],
            |r| r.get(0),
        )?;
        let messages = rows.into_iter().map(|(_, m)| m).collect();
        Ok((messages, oldest.filter(|_| remaining > 0)))
    }

    /// Page précédente de l'historique (messages plus anciens que
    /// `before_rowid`), ordre chronologique.
    pub fn load_older(
        &self,
        before_rowid: i64,
        limit: u32,
    ) -> rusqlite::Result<(Vec<ChatMessage>, Option<i64>)> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {} FROM messages WHERE id < ?1 ORDER BY id DESC LIMIT ?2",
            Self::MSG_COLS
        ))?;
        let mut rows: Vec<(i64, ChatMessage)> = stmt
            .query_map(params![before_rowid, limit], Self::row_to_message)?
            .collect::<Result<_, _>>()?;
        rows.reverse();
        let oldest = rows.first().map(|(id, _)| *id);
        let remaining: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE id < ?1",
            params![oldest.unwrap_or(0)],
            |r| r.get(0),
        )?;
        let messages = rows.into_iter().map(|(_, m)| m).collect();
        Ok((messages, oldest.filter(|_| remaining > 0)))
    }

    /// Identifiants de tous les médias référencés par l'historique complet
    /// (GC du cache disque `media/`).
    pub fn all_media_ids(&self) -> rusqlite::Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT json_extract(media, '$.id') FROM messages WHERE media IS NOT NULL")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, Option<String>>(0))?
            .filter_map(|r| r.ok().flatten())
            .collect();
        Ok(ids)
    }

    /// Charge l'état initial complet (fenêtre récente + tables annexes).
    pub fn load_all(&self, window: u32) -> LoadedState {
        let (messages, oldest_rowid) = self.load_recent(window).unwrap_or_default();

        let mut reactions: HashMap<u64, Vec<ReactionEntry>> = HashMap::new();
        if let Ok(mut stmt) = self
            .conn
            .prepare("SELECT message_hash, emoji, username FROM reactions ORDER BY rowid")
        {
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)? as u64,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            });
            if let Ok(rows) = rows {
                for (hash, emoji, user) in rows.flatten() {
                    let entries = reactions.entry(hash).or_default();
                    match entries.iter_mut().find(|e| e.emoji == emoji) {
                        Some(e) => e.users.push(user),
                        None => entries.push(ReactionEntry {
                            emoji,
                            users: vec![user],
                        }),
                    }
                }
            }
        }

        let mut read_counts = HashMap::new();
        if let Ok(mut stmt) = self.conn.prepare("SELECT username, count FROM read_counts") {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize))
            }) {
                read_counts = rows.flatten().collect();
            }
        }

        let mut groups = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare("SELECT data FROM groups") {
            if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                for data in rows.flatten() {
                    if let Ok(group) = serde_json::from_str::<Group>(&data) {
                        groups.push(group);
                    }
                }
            }
        }

        let mut peer_records = Vec::new();
        let mut peer_avatars = HashMap::new();
        let mut peer_keys = HashMap::new();
        if let Ok(mut stmt) = self
            .conn
            .prepare("SELECT username, alias, avatar, pubkey FROM peers")
        {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                    r.get::<_, Option<Vec<u8>>>(3)?,
                ))
            }) {
                for (username, alias, avatar, pubkey) in rows.flatten() {
                    if alias.is_some() {
                        peer_records.push(PeerRecord {
                            username: username.clone(),
                            alias,
                        });
                    }
                    if let Some(avatar) = avatar.filter(|a| !a.is_empty()) {
                        peer_avatars.insert(username.clone(), avatar);
                    }
                    if let Some(pubkey) = pubkey.filter(|k| !k.is_empty()) {
                        peer_keys.insert(username, pubkey);
                    }
                }
            }
        }

        let mut kv = HashMap::new();
        if let Ok(mut stmt) = self.conn.prepare("SELECT k, v FROM kv") {
            if let Ok(rows) = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
            }) {
                for (k, v) in rows.flatten() {
                    kv.insert(k, String::from_utf8_lossy(&v).into_owned());
                }
            }
        }

        LoadedState {
            messages,
            oldest_rowid,
            reactions,
            read_counts,
            groups,
            peer_records,
            peer_avatars,
            peer_keys,
            kv,
        }
    }

    // ── Migration JSON → SQLite ──────────────────────────────────────────

    /// Importe les anciens fichiers JSON puis les renomme en `.bak`.
    fn migrate_from_json(&mut self, base: &Path) {
        let read = |name: &str| -> Option<String> { std::fs::read_to_string(base.join(name)).ok() };
        let retire = |name: &str| {
            let p = base.join(name);
            if p.exists() {
                let _ = std::fs::rename(&p, base.join(format!("{name}.bak")));
            }
        };

        if let Some(content) = read("messages.json") {
            if let Ok(messages) = serde_json::from_str::<Vec<ChatMessage>>(&content) {
                let tx_ok = self.conn.execute_batch("BEGIN").is_ok();
                for msg in &messages {
                    let _ = self.insert_message(msg);
                }
                if tx_ok {
                    let _ = self.conn.execute_batch("COMMIT");
                }
                eprintln!(
                    "[storage] Migration : {} message(s) importé(s)",
                    messages.len()
                );
            }
        }
        if let Some(content) = read("reactions.json") {
            if let Ok(reactions) =
                serde_json::from_str::<HashMap<u64, Vec<ReactionEntry>>>(&content)
            {
                for (hash, entries) in &reactions {
                    let _ = self.replace_reactions(*hash, entries);
                }
            }
        }
        if let Some(content) = read("read_counts.json") {
            if let Ok(counts) = serde_json::from_str::<HashMap<String, usize>>(&content) {
                for (user, count) in &counts {
                    let _ = self.set_read_count(user, *count as u64);
                }
            }
        }
        if let Some(content) = read("groups.json") {
            if let Ok(groups) = serde_json::from_str::<Vec<Group>>(&content) {
                let _ = self.replace_groups(&groups);
            }
        }
        if let Some(content) = read("peer_records.json") {
            if let Ok(records) = serde_json::from_str::<Vec<PeerRecord>>(&content) {
                for r in &records {
                    let _ = self.upsert_peer_alias(&r.username, r.alias.as_deref());
                }
            }
        }
        if let Some(content) = read("peer_avatars.json") {
            if let Ok(avatars) = serde_json::from_str::<HashMap<String, Vec<u8>>>(&content) {
                for (user, png) in &avatars {
                    let _ = self.upsert_peer_avatar(user, Some(png));
                }
            }
        }

        for name in [
            "messages.json",
            "reactions.json",
            "read_counts.json",
            "groups.json",
            "peer_records.json",
            "peer_avatars.json",
        ] {
            retire(name);
        }
    }
}

/// Boucle du thread de stockage : applique les commandes dans l'ordre,
/// renvoie les pages d'historique à l'UI via `event_tx` (qui la réveille).
fn run(storage: Storage, rx: Receiver<StorageCmd>, event_tx: tokio::sync::mpsc::Sender<AppEvent>) {
    while let Ok(cmd) = rx.recv() {
        let result = match cmd {
            StorageCmd::InsertMessage(msg) => storage.insert_message(&msg),
            StorageCmd::DeleteConversation { me, conv } => {
                storage.delete_conversation(&me, conv.as_deref())
            }
            StorageCmd::DeleteMessageByMediaId(id) => storage.delete_by_media_id(&id),
            StorageCmd::ReplaceReactions { hash, entries } => {
                storage.replace_reactions(hash, &entries)
            }
            StorageCmd::SetReadCount { username, count } => {
                storage.set_read_count(&username, count)
            }
            StorageCmd::ReplaceGroups(groups) => storage.replace_groups(&groups),
            StorageCmd::UpsertPeerAlias { username, alias } => {
                storage.upsert_peer_alias(&username, alias.as_deref())
            }
            StorageCmd::UpsertPeerAvatar { username, avatar } => {
                storage.upsert_peer_avatar(&username, avatar.as_deref())
            }
            StorageCmd::UpsertPeerKey { username, pubkey } => {
                storage.upsert_peer_key(&username, &pubkey)
            }
            StorageCmd::SetKv { k, v } => storage.set_kv(&k, &v),
            StorageCmd::LoadOlder { before_rowid } => {
                match storage.load_older(before_rowid, OLDER_PAGE) {
                    Ok((messages, oldest_rowid)) => {
                        let _ = event_tx.blocking_send(AppEvent::OlderMessagesLoaded {
                            messages,
                            oldest_rowid,
                        });
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            StorageCmd::Flush(ack) => {
                let _ = ack.send(());
                Ok(())
            }
        };
        if let Err(e) = result {
            eprintln!("[storage] Erreur d'écriture : {e}");
        }
    }
}

/// Démarre le thread de stockage et renvoie l'émetteur de commandes.
pub fn spawn(
    storage: Storage,
    event_tx: tokio::sync::mpsc::Sender<AppEvent>,
) -> Sender<StorageCmd> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("abcom-storage".into())
        .spawn(move || run(storage, rx, event_tx))
        .expect("thread de stockage");
    tx
}

#[cfg(test)]
#[path = "../tests/test_app_storage.rs"]
mod tests;
