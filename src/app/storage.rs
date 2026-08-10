//! Persistance SQLite (remplace les fichiers JSON).
//!
//! Toute l'I/O passe par un **thread de stockage dédié** : les mutations de
//! [`AppState`](super::AppState) envoient des [`StorageCmd`] (O(1), aucune
//! sérialisation ni écriture disque dans le thread UI). L'historique complet
//! vit en base ; la mémoire ne charge qu'une fenêtre récente, étendue à la
//! demande par [`StorageCmd::LoadOlder`] (pagination façon Discord).
//!
//! Migration : au premier lancement avec une base absente, les anciens
//! fichiers JSON (`messages.json`, `reactions.json`, …) sont importés puis les
//! sources importées avec succès sont renommées en `.bak`.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, SyncSender};

use rusqlite::types::Type;
use rusqlite::{params, Connection};
use serde::de::DeserializeOwned;

use crate::message::{AppEvent, ChatMessage, Group, PeerRecord, ReactionEntry};

/// Fenêtre de messages chargée en mémoire au démarrage.
pub const INITIAL_WINDOW: u32 = 500;
/// Nombre maximal de résultats de recherche renvoyés à l'UI.
pub const SEARCH_LIMIT: u32 = 200;
/// Taille d'une page de chargement d'historique (scroll vers le haut).
pub const OLDER_PAGE: u32 = 100;
/// 2 : les salons sont désignés par un identifiant immuable — `to_user` porte
/// `#<id>` au lieu de `#<nom>`, et les hashs qui en dépendent sont recalculés.
const SCHEMA_VERSION: i64 = 2;

/// Nature d'un accusé nominatif persisté.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptKind {
    /// Le message a été reçu par ce pair (ACK).
    Delivered,
    /// Le message a été lu par ce pair.
    Read,
}

impl ReceiptKind {
    fn as_sql(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Read => "read",
        }
    }
}

/// Commandes du thread de stockage (FIFO : l'ordre des mutations est
/// préservé ; `Flush` répond une fois toutes les commandes précédentes
/// appliquées).
pub enum StorageCmd {
    InsertMessage(ChatMessage),
    /// Efface une conversation : `None` = fil « Tous » (broadcast),
    /// `Some("#nom")` = salon de groupe, `Some(pair)` = conversation privée.
    DeleteConversation {
        me: String,
        conv: Option<String>,
    },
    /// Migre l'historique d'un salon renommé (`to_user` : ancienne clé
    /// `#ancien` vers la nouvelle `#nouveau`).
    RenameConversation {
        old: String,
        new: String,
    },
    DeleteMessageByMediaId(String),
    /// Remplace l'ensemble des réactions d'un message (vide = suppression).
    ReplaceReactions {
        hash: u64,
        entries: Vec<ReactionEntry>,
    },
    /// Dernier message entrant marqué lu d'une conversation.
    SetReadMark {
        username: String,
        message_hash: u64,
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
    /// Désépinglage explicite : ré-appairage demandé par l'utilisateur après
    /// un changement de clé (réinstallation d'un pair).
    DeletePeerKey {
        username: String,
    },
    /// Message mis de côté pour un destinataire hors ligne.
    EnqueueOutbox {
        hash: u64,
        to_peer: String,
        message: ChatMessage,
    },
    /// Message de la file hors-ligne effectivement remis.
    DequeueOutbox {
        hash: u64,
    },
    /// Compaction de la base (VACUUM + ANALYZE), à la demande de l'utilisateur.
    Compact,
    /// Recherche plein texte ; le résultat revient par `AppEvent::SearchResults`.
    Search {
        query: String,
    },
    /// Export texte d'une conversation vers un fichier choisi par l'utilisateur.
    ExportConversation {
        me: String,
        conv: Option<String>,
        path: std::path::PathBuf,
    },
    /// Accusé nominatif livré/lu reçu d'un pair.
    AddReceipt {
        hash: u64,
        username: String,
        kind: ReceiptKind,
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
    pub read_marks: HashMap<String, u64>,
    pub groups: Vec<Group>,
    pub peer_records: Vec<PeerRecord>,
    pub peer_avatars: HashMap<String, Vec<u8>>,
    pub peer_keys: HashMap<String, Vec<u8>>,
    /// Messages en attente d'un destinataire hors ligne, par hash.
    pub outbox: HashMap<u64, (String, ChatMessage)>,
    /// Accusés de livraison nominatifs, par hash de message.
    pub delivered_receipts: HashMap<u64, HashSet<String>>,
    /// Accusés de lecture nominatifs, par hash de message.
    pub read_receipts: HashMap<u64, HashSet<String>>,
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
        let mut conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        // Attendre plutôt que renvoyer SQLITE_BUSY si une autre connexion écrit.
        conn.pragma_update(None, "busy_timeout", 5_000)?;
        // 64 Mo de cache et lecture par mmap : notre chemin chaud est la
        // pagination de l'historique, faite de lectures répétées.
        conn.pragma_update(None, "cache_size", -64_000)?;
        conn.pragma_update(None, "mmap_size", 268_435_456)?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        let tx = conn.transaction()?;
        tx.execute_batch(
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
            -- Toutes les requêtes de conversation filtrent sur `to_user` : sans
            -- cet index, chacune parcourt la table entière.
            CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(to_user, id);
            CREATE TABLE IF NOT EXISTS reactions (
                message_hash INTEGER NOT NULL,
                emoji        TEXT    NOT NULL,
                username     TEXT    NOT NULL,
                PRIMARY KEY (message_hash, emoji, username)
            );
            -- Repère « lu jusqu'à » : un hash de message, pas un compteur.
            -- Un compteur devenait faux dès qu'une purge changeait l'ensemble
            -- des messages présents.
            CREATE TABLE IF NOT EXISTS read_marks (
                username     TEXT PRIMARY KEY,
                message_hash INTEGER NOT NULL
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
            CREATE TABLE IF NOT EXISTS kv (k TEXT PRIMARY KEY, v BLOB);
            -- Index plein texte. `content=''` n'y stocke pas une seconde copie
            -- des messages, `contentless_delete=1` autorise quand même la
            -- suppression — indispensable, on efface des conversations.
            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts
                USING fts5(content, content='', contentless_delete=1);
            CREATE TRIGGER IF NOT EXISTS messages_fts_insert AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
            END;
            CREATE TRIGGER IF NOT EXISTS messages_fts_delete AFTER DELETE ON messages BEGIN
                DELETE FROM messages_fts WHERE rowid = old.id;
            END;
            CREATE TRIGGER IF NOT EXISTS messages_fts_update AFTER UPDATE OF content ON messages BEGIN
                DELETE FROM messages_fts WHERE rowid = old.id;
                INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
            END;
            -- Accusés nominatifs ; la clé primaire rend l'insertion idempotente.
            -- Messages en attente d'un destinataire hors ligne.
            CREATE TABLE IF NOT EXISTS outbox (
                hash    INTEGER PRIMARY KEY,
                to_peer TEXT    NOT NULL,
                message TEXT    NOT NULL
            );
            CREATE TABLE IF NOT EXISTS receipts (
                message_hash INTEGER NOT NULL,
                username     TEXT    NOT NULL,
                kind         TEXT    NOT NULL,
                PRIMARY KEY (message_hash, username, kind)
            );",
        )?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 1 {
            let has_nonce: bool = tx.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_table_info('messages') WHERE name = 'nonce'
                )",
                [],
                |row| row.get(0),
            )?;
            if !has_nonce {
                tx.execute("ALTER TABLE messages ADD COLUMN nonce INTEGER", [])?;
            }
        }
        if version < 2 {
            Self::migrate_groups_to_ids(&tx)?;
        }
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;

        let mut storage = Self { conn };
        storage.backfill_search_index()?;
        // Les sources déjà importées ont été renommées en `.bak`. Réessayer à
        // chaque ouverture permet de reprendre proprement une migration dont
        // une source ou une écriture avait échoué au premier lancement.
        storage.migrate_from_json(base)?;
        storage.purge_orphan_receipts()?;
        purge_legacy_backups(base);
        Ok(storage)
    }

    /// Bascule les salons du nom vers l'identifiant (schéma v2).
    ///
    /// `to_user` portait `#<nom>` et ce nom entrait dans le hash des messages :
    /// un renommage laissait derrière lui des réactions, des accusés et un
    /// repère de lecture accrochés à des hashs que plus aucun message ne
    /// produisait. On réécrit donc la clé en `#<id>`, on recalcule les hashs
    /// concernés et on reporte les tables qui s'y réfèrent.
    fn migrate_groups_to_ids(tx: &Connection) -> rusqlite::Result<()> {
        let groups: Vec<Group> = {
            let mut stmt = tx.prepare("SELECT data FROM groups")?;
            let rows = stmt
                .query_map([], |row| {
                    let data = row.get::<_, String>(0)?;
                    serde_json::from_str::<Group>(&data)
                        .map_err(|error| Self::serde_from_sql(0, error))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };

        for mut group in groups {
            let old_key = format!("#{}", group.name);
            group.ensure_id();
            let new_key = format!("#{}", group.id);
            if old_key == new_key {
                continue;
            }

            // Le salon lui-même : on persiste l'identifiant dérivé.
            let data = serde_json::to_string(&group).map_err(Self::serde_to_sql)?;
            tx.execute(
                "UPDATE groups SET data = ?2 WHERE name = ?1",
                params![group.name, data],
            )?;

            // Messages du salon : nouvelle clé et nouveau hash.
            let rows: Vec<(i64, i64, ChatMessage)> = {
                let mut stmt = tx.prepare(
                    "SELECT id, hash, from_user, content, timestamp, ts_epoch, media, reply_to,
                            nonce
                     FROM messages WHERE to_user = ?1",
                )?;
                let rows = stmt
                    .query_map(params![old_key], |row| {
                        let media = row
                            .get::<_, Option<String>>(6)?
                            .map(|raw| serde_json::from_str(&raw))
                            .transpose()
                            .map_err(|error| Self::serde_from_sql(6, error))?;
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            ChatMessage {
                                from: row.get::<_, String>(2)?,
                                content: row.get::<_, String>(3)?,
                                timestamp: row.get::<_, String>(4)?,
                                timestamp_epoch: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                                // La nouvelle clé : c'est elle qui change le hash.
                                to_user: Some(new_key.clone()),
                                media,
                                reply_to: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                                nonce: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
                            },
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            };

            for (row_id, old_hash, message) in rows {
                let new_hash = message.stable_hash() as i64;
                tx.execute(
                    "UPDATE messages SET to_user = ?2, hash = ?3 WHERE id = ?1",
                    params![row_id, new_key, new_hash],
                )?;
                if new_hash != old_hash {
                    // `OR REPLACE` : deux hashs distincts peuvent converger vers
                    // la même ligne cible, la clé primaire absorbe le doublon.
                    tx.execute(
                        "UPDATE OR REPLACE reactions SET message_hash = ?2 WHERE message_hash = ?1",
                        params![old_hash, new_hash],
                    )?;
                    tx.execute(
                        "UPDATE OR REPLACE receipts SET message_hash = ?2 WHERE message_hash = ?1",
                        params![old_hash, new_hash],
                    )?;
                    tx.execute(
                        "UPDATE read_marks SET message_hash = ?2 WHERE message_hash = ?1",
                        params![old_hash, new_hash],
                    )?;
                }
            }

            // Repère de lecture : il est indexé par la clé de conversation.
            tx.execute(
                "UPDATE OR REPLACE read_marks SET username = ?2 WHERE username = ?1",
                params![old_key, new_key],
            )?;
        }
        Ok(())
    }

    fn serde_to_sql(error: serde_json::Error) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(error))
    }

    fn serde_from_sql(column: usize, error: serde_json::Error) -> rusqlite::Error {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
    }

    fn utf8_from_sql(column: usize, error: std::string::FromUtf8Error) -> rusqlite::Error {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Blob, Box::new(error))
    }

    fn insert_message_on(conn: &Connection, msg: &ChatMessage) -> rusqlite::Result<()> {
        let media = msg
            .media
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(Self::serde_to_sql)?;
        conn.execute(
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

    fn replace_reactions_on(
        conn: &Connection,
        hash: u64,
        entries: &[ReactionEntry],
    ) -> rusqlite::Result<()> {
        conn.execute(
            "DELETE FROM reactions WHERE message_hash = ?1",
            params![hash as i64],
        )?;
        let mut stmt = conn.prepare_cached(
            "INSERT OR IGNORE INTO reactions (message_hash, emoji, username) VALUES (?1, ?2, ?3)",
        )?;
        for entry in entries {
            for user in &entry.users {
                stmt.execute(params![hash as i64, entry.emoji, user])?;
            }
        }
        Ok(())
    }

    fn replace_groups_on(conn: &Connection, groups: &[Group]) -> rusqlite::Result<()> {
        let serialized = groups
            .iter()
            .map(|group| {
                serde_json::to_string(group)
                    .map(|data| (&group.name, data))
                    .map_err(Self::serde_to_sql)
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;

        conn.execute("DELETE FROM groups", [])?;
        let mut stmt = conn.prepare_cached("INSERT INTO groups (name, data) VALUES (?1, ?2)")?;
        for (name, data) in serialized {
            stmt.execute(params![name, data])?;
        }
        Ok(())
    }

    // ── Écritures ────────────────────────────────────────────────────────

    pub fn insert_message(&self, msg: &ChatMessage) -> rusqlite::Result<()> {
        Self::insert_message_on(&self.conn, msg)
    }

    /// Force la version de schéma, pour rejouer une migration dans les tests.
    #[cfg(test)]
    pub fn set_user_version(&self, version: i64) -> rusqlite::Result<()> {
        self.conn.pragma_update(None, "user_version", version)
    }

    /// Insertion groupée : un seul commit WAL par rafale au lieu d'un par message.
    pub fn insert_messages(&mut self, msgs: &[ChatMessage]) -> rusqlite::Result<()> {
        match msgs {
            [] => Ok(()),
            [msg] => Self::insert_message_on(&self.conn, msg),
            _ => {
                let tx = self.conn.transaction()?;
                for msg in msgs {
                    Self::insert_message_on(&tx, msg)?;
                }
                tx.commit()
            }
        }
    }

    pub fn delete_conversation(&self, me: &str, conv: Option<&str>) -> rusqlite::Result<()> {
        match conv {
            None => {
                self.conn
                    .execute("DELETE FROM messages WHERE to_user IS NULL", [])?;
            }
            // Salon de groupe : tous les messages portent la clé en `to_user`,
            // quel que soit l'auteur.
            Some(conv) if conv.starts_with('#') => {
                self.conn
                    .execute("DELETE FROM messages WHERE to_user = ?1", params![conv])?;
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

    pub fn rename_conversation(&self, old: &str, new: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE messages SET to_user = ?2 WHERE to_user = ?1",
            params![old, new],
        )?;
        Ok(())
    }

    pub fn delete_by_media_id(&self, media_id: &str) -> rusqlite::Result<()> {
        // Le média est stocké en JSON : filtre sur l'id exact.
        self.conn.execute(
            "DELETE FROM messages
             WHERE media IS NOT NULL AND media ->> 'id' = ?1",
            params![media_id],
        )?;
        Ok(())
    }

    pub fn replace_reactions(
        &mut self,
        hash: u64,
        entries: &[ReactionEntry],
    ) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        Self::replace_reactions_on(&tx, hash, entries)?;
        tx.commit()
    }

    pub fn set_read_mark(&self, username: &str, message_hash: u64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO read_marks (username, message_hash) VALUES (?1, ?2)
             ON CONFLICT(username) DO UPDATE SET message_hash = excluded.message_hash",
            params![username, message_hash as i64],
        )?;
        Ok(())
    }

    pub fn replace_groups(&mut self, groups: &[Group]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        Self::replace_groups_on(&tx, groups)?;
        tx.commit()
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

    /// Remplit l'index plein texte pour les messages antérieurs à sa création.
    fn backfill_search_index(&mut self) -> rusqlite::Result<()> {
        let indexed: i64 = self
            .conn
            .query_row("SELECT count(*) FROM messages_fts", [], |row| row.get(0))?;
        if indexed > 0 {
            return Ok(());
        }
        let total: i64 = self
            .conn
            .query_row("SELECT count(*) FROM messages", [], |row| row.get(0))?;
        if total == 0 {
            return Ok(());
        }
        self.conn.execute(
            "INSERT INTO messages_fts(rowid, content) SELECT id, content FROM messages",
            [],
        )?;
        tracing::info!("index de recherche construit sur {total} message(s)");
        Ok(())
    }

    /// Recherche plein texte dans l'historique, du plus récent au plus ancien.
    pub fn search(&self, query: &str, limit: u32) -> rusqlite::Result<Vec<ChatMessage>> {
        let Some(expression) = fts_expression(query) else {
            return Ok(Vec::new());
        };
        let sql = format!(
            "SELECT {} FROM messages
             WHERE id IN (SELECT rowid FROM messages_fts WHERE messages_fts MATCH ?1)
             ORDER BY id DESC LIMIT ?2",
            Self::MSG_COLS
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![expression, limit], Self::row_to_message)?;
        rows.map(|row| row.map(|(_, message)| message)).collect()
    }

    /// Rafraîchit les statistiques du planificateur de requêtes.
    ///
    /// Recommandation de l'amont pour une connexion de longue durée — le thread
    /// de stockage vit aussi longtemps que l'application. Sans cela, les plans
    /// sont choisis sur des statistiques périmées, ce qui compte d'autant plus
    /// avec l'index de conversation.
    pub fn optimize(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch("PRAGMA optimize;")
    }

    /// Compacte la base et réindexe : le fichier ne récupère jamais seul
    /// l'espace des conversations effacées.
    pub fn compact(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch("VACUUM; ANALYZE;")
    }

    /// Taille du fichier de base et nombre de messages, pour l'affichage Paramètres.
    pub fn footprint(&self, base: &Path) -> rusqlite::Result<(u64, u64)> {
        let messages: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?;
        let bytes = std::fs::metadata(base.join("abcom.db"))
            .map(|m| m.len())
            .unwrap_or(0);
        Ok((bytes, messages as u64))
    }

    /// Exporte une conversation en texte lisible (portabilité local-first).
    ///
    /// `conv` suit la convention du reste du stockage : `None` = « Tous »,
    /// `Some("#salon")`, `Some("pair")`.
    pub fn export_conversation(&self, me: &str, conv: Option<&str>) -> rusqlite::Result<String> {
        let mut out = String::new();
        for message in self.conversation_messages(me, conv)? {
            let stamp = message
                .timestamp_epoch
                .and_then(|e| chrono::DateTime::from_timestamp(e as i64, 0))
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| message.timestamp.clone());
            out.push_str(&format!(
                "[{stamp}] {} : {}\n",
                message.from, message.content
            ));
            if let Some(media) = &message.media {
                out.push_str(&format!("    (pièce jointe : {})\n", media.filename));
            }
        }
        Ok(out)
    }

    fn conversation_messages(
        &self,
        me: &str,
        conv: Option<&str>,
    ) -> rusqlite::Result<Vec<ChatMessage>> {
        let (clause, params): (&str, Vec<String>) = match conv {
            None => ("to_user IS NULL", Vec::new()),
            Some(group) if group.starts_with('#') => ("to_user = ?1", vec![group.to_string()]),
            Some(peer) => (
                "(from_user = ?1 AND to_user = ?2) OR (from_user = ?2 AND to_user = ?1)",
                vec![peer.to_string(), me.to_string()],
            ),
        };
        let sql = format!(
            "SELECT {} FROM messages WHERE {clause} ORDER BY id",
            Self::MSG_COLS
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params), Self::row_to_message)?;
        rows.map(|row| row.map(|(_, message)| message)).collect()
    }

    /// Met un message de côté pour un destinataire hors ligne.
    pub fn enqueue_outbox(
        &self,
        hash: u64,
        to_peer: &str,
        message: &ChatMessage,
    ) -> rusqlite::Result<()> {
        let payload = serde_json::to_string(message).map_err(Self::serde_to_sql)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO outbox (hash, to_peer, message) VALUES (?1, ?2, ?3)",
            params![hash as i64, to_peer, payload],
        )?;
        Ok(())
    }

    pub fn dequeue_outbox(&self, hash: u64) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM outbox WHERE hash = ?1", params![hash as i64])?;
        Ok(())
    }

    /// Enregistre un accusé nominatif ; idempotent, un pair peut réémettre le sien.
    pub fn add_receipt(
        &self,
        hash: u64,
        username: &str,
        kind: ReceiptKind,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO receipts (message_hash, username, kind) VALUES (?1, ?2, ?3)",
            params![hash as i64, username, kind.as_sql()],
        )?;
        Ok(())
    }

    /// Supprime les accusés dont le message a disparu, sinon la table ne fait que croître.
    pub fn purge_orphan_receipts(&self) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM receipts
             WHERE message_hash NOT IN (SELECT hash FROM messages)",
            [],
        )?;
        Ok(())
    }

    /// Retire la clé épinglée d'un pair sans toucher au reste de sa fiche
    /// (alias, avatar) : la prochaine connexion ré-épinglera.
    pub fn delete_peer_key(&self, username: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE peers SET pubkey = NULL WHERE username = ?1",
            params![username],
        )?;
        Ok(())
    }

    // ── Lectures ─────────────────────────────────────────────────────────

    fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, ChatMessage)> {
        let rowid: i64 = row.get(0)?;
        let media: Option<String> = row.get(6)?;
        let media = media
            .map(|value| {
                serde_json::from_str(&value).map_err(|error| Self::serde_from_sql(6, error))
            })
            .transpose()?;
        Ok((
            rowid,
            ChatMessage {
                from: row.get(1)?,
                to_user: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get(4)?,
                timestamp_epoch: row.get::<_, Option<i64>>(5)?.map(|e| e as u64),
                media,
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
        let mut stmt = self.conn.prepare_cached(&format!(
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
        let mut stmt = self.conn.prepare_cached(&format!(
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
            .prepare_cached("SELECT media ->> 'id' FROM messages WHERE media IS NOT NULL")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, Option<String>>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(ids)
    }

    /// Charge l'état initial complet (fenêtre récente + tables annexes).
    pub fn load_all(&self, window: u32) -> rusqlite::Result<LoadedState> {
        let (messages, oldest_rowid) = self.load_recent(window)?;

        let mut reactions: HashMap<u64, Vec<ReactionEntry>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT message_hash, emoji, username FROM reactions ORDER BY rowid")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)? as u64,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (hash, emoji, user) = row?;
            let entries = reactions.entry(hash).or_default();
            match entries.iter_mut().find(|e| e.emoji == emoji) {
                Some(e) => e.users.push(user),
                None => entries.push(ReactionEntry {
                    emoji,
                    users: vec![user],
                }),
            }
        }

        let mut outbox: HashMap<u64, (String, ChatMessage)> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT hash, to_peer, message FROM outbox")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)? as u64,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (hash, to_peer, payload) = row?;
            let message: ChatMessage =
                serde_json::from_str(&payload).map_err(|e| Self::serde_from_sql(2, e))?;
            outbox.insert(hash, (to_peer, message));
        }

        // Accusés nominatifs : deux maps hash → ensemble de pairs.
        let mut delivered_receipts: HashMap<u64, HashSet<String>> = HashMap::new();
        let mut read_receipts: HashMap<u64, HashSet<String>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT message_hash, username, kind FROM receipts")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)? as u64,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (hash, username, kind) = row?;
            let target = if kind == "read" {
                &mut read_receipts
            } else {
                &mut delivered_receipts
            };
            target.entry(hash).or_default().insert(username);
        }

        let mut stmt = self
            .conn
            .prepare("SELECT username, message_hash FROM read_marks")?;
        let read_marks = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })?
            .collect::<rusqlite::Result<HashMap<_, _>>>()?;

        let mut stmt = self.conn.prepare("SELECT data FROM groups")?;
        let groups = stmt
            .query_map([], |row| {
                let data = row.get::<_, String>(0)?;
                serde_json::from_str::<Group>(&data)
                    .map(|mut group| {
                        // Salon enregistré avant l'introduction des
                        // identifiants : on le dérive, à l'identique chez tous
                        // les membres.
                        group.ensure_id();
                        group
                    })
                    .map_err(|error| Self::serde_from_sql(0, error))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut peer_records = Vec::new();
        let mut peer_avatars = HashMap::new();
        let mut peer_keys = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT username, alias, avatar, pubkey FROM peers")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<Vec<u8>>>(2)?,
                r.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })?;
        for row in rows {
            let (username, alias, avatar, pubkey) = row?;
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

        let mut stmt = self.conn.prepare("SELECT k, v FROM kv")?;
        let rows = stmt.query_map([], |r| {
            let key = r.get::<_, String>(0)?;
            let value = String::from_utf8(r.get::<_, Vec<u8>>(1)?)
                .map_err(|error| Self::utf8_from_sql(1, error))?;
            Ok((key, value))
        })?;
        let mut kv = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            kv.insert(key, value);
        }

        Ok(LoadedState {
            messages,
            oldest_rowid,
            reactions,
            read_marks,
            groups,
            peer_records,
            peer_avatars,
            peer_keys,
            outbox,
            delivered_receipts,
            read_receipts,
            kv,
        })
    }

    // ── Migration JSON → SQLite ──────────────────────────────────────────

    fn read_legacy_json<T: DeserializeOwned>(base: &Path, name: &str) -> Option<T> {
        let path = base.join(name);
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => {
                tracing::error!(
                    "migration : lecture de {} impossible : {error}",
                    path.display()
                );
                return None;
            }
        };
        match serde_json::from_str(&content) {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::error!(
                    "migration : désérialisation de {} impossible : {error}",
                    path.display()
                );
                None
            }
        }
    }

    /// Importe les anciens fichiers JSON dans une transaction puis renomme en
    /// `.bak` uniquement les sources dont l'import a été validé par le commit.
    fn migrate_from_json(&mut self, base: &Path) -> rusqlite::Result<()> {
        let legacy_names = [
            "messages.json",
            "reactions.json",
            "read_counts.json",
            "groups.json",
            "peer_records.json",
            "peer_avatars.json",
        ];
        let mut already_imported = std::collections::HashSet::new();
        for name in legacy_names {
            let key = format!("legacy_import:{name}");
            let done: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM kv WHERE k = ?1)",
                params![key],
                |row| row.get(0),
            )?;
            if done {
                already_imported.insert(name);
            }
        }

        let messages = (!already_imported.contains("messages.json"))
            .then(|| Self::read_legacy_json::<Vec<ChatMessage>>(base, "messages.json"))
            .flatten();
        let reactions = (!already_imported.contains("reactions.json"))
            .then(|| {
                Self::read_legacy_json::<HashMap<u64, Vec<ReactionEntry>>>(base, "reactions.json")
            })
            .flatten();
        let read_counts = (!already_imported.contains("read_counts.json"))
            .then(|| Self::read_legacy_json::<HashMap<String, usize>>(base, "read_counts.json"))
            .flatten();
        let groups = (!already_imported.contains("groups.json"))
            .then(|| Self::read_legacy_json::<Vec<Group>>(base, "groups.json"))
            .flatten();
        let peer_records = (!already_imported.contains("peer_records.json"))
            .then(|| Self::read_legacy_json::<Vec<PeerRecord>>(base, "peer_records.json"))
            .flatten();
        let peer_avatars = (!already_imported.contains("peer_avatars.json"))
            .then(|| Self::read_legacy_json::<HashMap<String, Vec<u8>>>(base, "peer_avatars.json"))
            .flatten();

        let tx = self.conn.transaction()?;
        let mut imported = Vec::new();
        if let Some(messages) = messages {
            for message in &messages {
                Self::insert_message_on(&tx, message)?;
            }
            tracing::info!("migration : {} message(s) importé(s)", messages.len());
            imported.push("messages.json");
        }
        if let Some(reactions) = reactions {
            for (hash, entries) in &reactions {
                Self::replace_reactions_on(&tx, *hash, entries)?;
            }
            imported.push("reactions.json");
        }
        if read_counts.is_some() {
            // Un ancien compteur ne se convertit pas en repère de message :
            // on marque la source comme importée sans rien inventer, la
            // conversation repassera simplement « non lue » une fois.
            imported.push("read_counts.json");
        }
        if let Some(groups) = groups {
            Self::replace_groups_on(&tx, &groups)?;
            imported.push("groups.json");
        }
        if let Some(records) = peer_records {
            for record in &records {
                tx.execute(
                    "INSERT INTO peers (username, alias) VALUES (?1, ?2)
                     ON CONFLICT(username) DO UPDATE SET alias = excluded.alias",
                    params![record.username, record.alias],
                )?;
            }
            imported.push("peer_records.json");
        }
        if let Some(avatars) = peer_avatars {
            for (user, avatar) in &avatars {
                tx.execute(
                    "INSERT INTO peers (username, avatar) VALUES (?1, ?2)
                     ON CONFLICT(username) DO UPDATE SET avatar = excluded.avatar",
                    params![user, avatar],
                )?;
            }
            imported.push("peer_avatars.json");
        }
        for name in &imported {
            tx.execute(
                "INSERT OR REPLACE INTO kv (k, v) VALUES (?1, ?2)",
                params![format!("legacy_import:{name}"), b"1".as_slice()],
            )?;
        }
        tx.commit()?;

        for name in already_imported.into_iter().chain(imported) {
            let source = base.join(name);
            if !source.exists() {
                continue;
            }
            let backup = base.join(format!("{name}.bak"));
            if let Err(error) = std::fs::rename(&source, &backup) {
                tracing::error!(
                    "migration : renommage de {} en {} impossible : {error}",
                    source.display(),
                    backup.display()
                );
            }
        }
        Ok(())
    }
}

/// Traduit une saisie utilisateur en expression FTS5 sûre.
///
/// La syntaxe FTS5 a ses propres opérateurs : une saisie brute peut être
/// invalide (guillemet non fermé) et faire échouer la requête. On cite donc
/// chaque terme, ce qui les traite littéralement, et le dernier reçoit un `*`
/// pour la recherche au fil de la frappe.
fn fts_expression(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    let (last, head) = terms.split_last()?;
    let mut expression = head.join(" ");
    if !expression.is_empty() {
        expression.push(' ');
    }
    expression.push_str(last);
    expression.push('*');
    Some(expression)
}

/// Délai de conservation des sauvegardes de la migration JSON → SQLite.
pub const LEGACY_BACKUP_TTL: std::time::Duration =
    std::time::Duration::from_secs(30 * 24 * 60 * 60);

/// Plafond d'un lot : au-delà on commite pour ne pas retarder la suite de la file.
const INSERT_BATCH_MAX: usize = 256;

/// Supprime les `*.json.bak` de migration passé [`LEGACY_BACKUP_TTL`] : sinon l'historique est dupliqué à vie.
pub fn purge_legacy_backups(base: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.contains(".json.bak") {
            continue;
        }
        let expired = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|modified| modified.elapsed().is_ok_and(|age| age > LEGACY_BACKUP_TTL))
            .unwrap_or(false);
        if expired {
            match std::fs::remove_file(entry.path()) {
                Ok(()) => tracing::info!("sauvegarde de migration purgée : {name}"),
                Err(error) => tracing::warn!("purge de {name} impossible : {error}"),
            }
        }
    }
}

/// Boucle du thread de stockage : applique les commandes dans l'ordre,
/// renvoie les pages d'historique à l'UI via `event_tx` (qui la réveille).
fn run(
    mut storage: Storage,
    rx: Receiver<StorageCmd>,
    event_tx: tokio::sync::mpsc::Sender<AppEvent>,
) {
    // Commande qui a interrompu un lot : à rejouer avant toute lecture, sinon l'ordre change.
    let mut deferred: Option<StorageCmd> = None;
    loop {
        let cmd = match deferred.take() {
            Some(cmd) => cmd,
            None => match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
            },
        };

        // Rafale : on draine la file pour n'ouvrir qu'une transaction.
        if let StorageCmd::InsertMessage(first) = cmd {
            let mut batch = vec![first];
            while batch.len() < INSERT_BATCH_MAX {
                match rx.try_recv() {
                    Ok(StorageCmd::InsertMessage(msg)) => batch.push(msg),
                    Ok(other) => {
                        deferred = Some(other);
                        break;
                    }
                    Err(_) => break,
                }
            }
            if let Err(e) = storage.insert_messages(&batch) {
                tracing::error!("erreur d'écriture : {e}");
            }
            continue;
        }

        let result = match cmd {
            // Traité juste au-dessus.
            StorageCmd::InsertMessage(msg) => storage.insert_message(&msg),
            StorageCmd::DeleteConversation { me, conv } => {
                storage.delete_conversation(&me, conv.as_deref())
            }
            StorageCmd::RenameConversation { old, new } => storage.rename_conversation(&old, &new),
            StorageCmd::DeleteMessageByMediaId(id) => storage.delete_by_media_id(&id),
            StorageCmd::ReplaceReactions { hash, entries } => {
                storage.replace_reactions(hash, &entries)
            }
            StorageCmd::SetReadMark {
                username,
                message_hash,
            } => storage.set_read_mark(&username, message_hash),
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
            StorageCmd::DeletePeerKey { username } => storage.delete_peer_key(&username),
            StorageCmd::EnqueueOutbox {
                hash,
                to_peer,
                message,
            } => storage.enqueue_outbox(hash, &to_peer, &message),
            StorageCmd::DequeueOutbox { hash } => storage.dequeue_outbox(hash),
            StorageCmd::Compact => storage.compact(),
            StorageCmd::Search { query } => match storage.search(&query, SEARCH_LIMIT) {
                Ok(messages) => {
                    let _ = event_tx.blocking_send(AppEvent::SearchResults { query, messages });
                    Ok(())
                }
                Err(error) => Err(error),
            },
            StorageCmd::ExportConversation { me, conv, path } => {
                match storage.export_conversation(&me, conv.as_deref()) {
                    Ok(text) => {
                        if let Err(error) = std::fs::write(&path, text) {
                            tracing::error!("export impossible : {error}");
                        } else {
                            tracing::info!("conversation exportée vers {}", path.display());
                        }
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            }
            StorageCmd::AddReceipt {
                hash,
                username,
                kind,
            } => storage.add_receipt(hash, &username, kind),
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
            tracing::error!("erreur d'écriture : {e}");
        }
    }

    // Canal fermé : l'application s'arrête, on laisse le planificateur des
    // statistiques à jour pour le prochain démarrage.
    if let Err(e) = storage.optimize() {
        tracing::warn!("PRAGMA optimize : {e}");
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
