use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::message::{ChatMessage, Group, PeerRecord, ReactionEntry};

mod avatar;
mod conversation;
mod groups;
pub mod media;
mod messages;
mod peers;
mod reactions;
mod receipts;
pub mod storage;
mod transfers;
mod typing;

pub use conversation::ConversationId;
pub use peers::Peer;
pub use receipts::{PendingMessage, ReceiptDetail};
pub use storage::{LoadedState, Storage, StorageCmd};
pub use transfers::TransferTarget;

pub struct AppState {
    pub my_username: String,
    pub peers: Vec<Peer>,
    pub messages: Vec<ChatMessage>,
    pub groups: Vec<Group>,
    pub selected_conversation: Option<String>,
    pub typing_users: HashMap<String, SystemTime>,
    pub read_counts: HashMap<String, usize>,
    pub read_receipts: HashMap<u64, HashSet<String>>,
    /// Qui a envoyé un ACK de livraison, par message (détail « reçu par »
    /// des salons et de « Tous »).
    pub delivered_receipts: HashMap<u64, HashSet<String>>,
    pub pending_messages: HashMap<u64, PendingMessage>,
    /// Messages privés dont toutes les tentatives de livraison ont échoué.
    pub failed_messages: HashMap<u64, String>,
    pub peer_records: Vec<PeerRecord>,
    /// Avatar local (octets PNG normalisés), `None` si non défini.
    pub my_avatar: Option<Vec<u8>>,
    /// Avatars des pairs, indexés par nom d'utilisateur.
    pub peer_avatars: HashMap<String, Vec<u8>>,
    /// Réactions emoji par message, indexées par `AppState::message_hash`.
    pub reactions: HashMap<u64, Vec<ReactionEntry>>,
    /// Préférences persistées (notifications, autostart…), miroir de la
    /// table kv.
    pub kv: HashMap<String, String>,
    /// Compteur incrémenté à chaque mutation du **contenu** (messages,
    /// réactions, accusés, avatars, alias). Le cache du fil ne se
    /// reconstruit que lorsqu'il change.
    pub content_generation: u64,
    /// Compteur incrémenté sur les changements de **présence** (pairs en
    /// ligne/hors ligne, frappe) : n'invalide que la barre latérale, pas le
    /// fil (la frappe d'un pair ne doit pas reconstruire 500 lignes).
    pub presence_generation: u64,
    /// Émetteur vers le thread de stockage SQLite (`None` en tests isolés :
    /// les mutations restent purement en mémoire).
    storage: Option<std::sync::mpsc::Sender<StorageCmd>>,
    /// Messages en attente d'un destinataire hors ligne, par hash (persistés).
    pub outbox: HashMap<u64, (String, ChatMessage)>,
    /// Messages entrants par conversation, dérivé une fois par génération (évite un O(n·m) par frame).
    incoming_counts: std::cell::RefCell<(u64, HashMap<String, usize>)>,
    /// rowid du plus ancien message chargé en mémoire (pagination) ;
    /// `None` = tout l'historique est déjà en mémoire.
    pub oldest_loaded_rowid: Option<i64>,
    /// Plafond de messages conservés en mémoire ; grandit quand l'historique
    /// est paginé vers le haut, borné par [`Self::MAX_WINDOW`].
    history_cap: usize,
    avatar_path: PathBuf,
    media_dir: PathBuf,
}

impl AppState {
    /// Fenêtre mémoire maximale (au-delà, la pagination vers le haut
    /// s'arrête : l'historique plus ancien reste en base).
    pub const MAX_WINDOW: usize = 2000;

    pub fn selected_conversation_id(&self) -> ConversationId {
        ConversationId::from_key(self.selected_conversation.as_deref())
    }

    pub fn new(
        username: String,
        loaded: LoadedState,
        storage: Option<std::sync::mpsc::Sender<StorageCmd>>,
    ) -> Self {
        let base = crate::config::data_dir();
        let history_cap = loaded.messages.len().max(storage::INITIAL_WINDOW as usize);

        let mut state = Self {
            my_username: username,
            peers: Vec::new(),
            messages: loaded.messages,
            groups: loaded.groups,
            selected_conversation: None,
            typing_users: HashMap::new(),
            read_counts: loaded.read_counts,
            // Relus depuis la base : coches et détail « … » survivent au redémarrage.
            read_receipts: loaded.read_receipts,
            delivered_receipts: loaded.delivered_receipts,
            outbox: loaded.outbox,
            pending_messages: HashMap::new(),
            failed_messages: HashMap::new(),
            peer_records: loaded.peer_records,
            my_avatar: None,
            peer_avatars: loaded.peer_avatars,
            reactions: loaded.reactions,
            kv: loaded.kv,
            content_generation: 0,
            presence_generation: 0,
            storage,
            // `u64::MAX` = jamais calculé.
            incoming_counts: std::cell::RefCell::new((u64::MAX, HashMap::new())),
            oldest_loaded_rowid: loaded.oldest_rowid,
            history_cap,
            avatar_path: base.join("avatar.png"),
            media_dir: base.join("media"),
        };

        state.load_avatar();
        state.restore_peers_from_history();
        state
    }

    /// Constructeur de test : aucun stockage, répertoire de données isolé.
    #[cfg(test)]
    pub fn new_with_base(username: &str, base: &std::path::Path) -> Self {
        Self {
            my_username: username.to_string(),
            peers: Vec::new(),
            messages: Vec::new(),
            groups: Vec::new(),
            selected_conversation: None,
            typing_users: HashMap::new(),
            read_counts: HashMap::new(),
            read_receipts: HashMap::new(),
            delivered_receipts: HashMap::new(),
            outbox: HashMap::new(),
            pending_messages: HashMap::new(),
            failed_messages: HashMap::new(),
            peer_records: Vec::new(),
            my_avatar: None,
            peer_avatars: HashMap::new(),
            reactions: HashMap::new(),
            kv: HashMap::new(),
            content_generation: 0,
            presence_generation: 0,
            storage: None,
            incoming_counts: std::cell::RefCell::new((u64::MAX, HashMap::new())),
            oldest_loaded_rowid: None,
            history_cap: storage::INITIAL_WINDOW as usize,
            avatar_path: base.join("avatar.png"),
            media_dir: base.join("media"),
        }
    }

    /// Mutation du contenu : invalide le cache du fil et de la barre latérale.
    pub fn bump_content(&mut self) {
        self.content_generation = self.content_generation.wrapping_add(1);
    }

    /// Mutation de présence (pairs, frappe) : n'invalide que la barre latérale.
    pub fn bump_presence(&mut self) {
        self.presence_generation = self.presence_generation.wrapping_add(1);
    }

    /// Lit une préférence booléenne persistée.
    pub fn pref_bool(&self, key: &str, default: bool) -> bool {
        self.kv.get(key).map(|v| v == "1").unwrap_or(default)
    }

    /// Écrit une préférence persistée (mémoire + table kv).
    pub fn set_pref(&mut self, key: &str, value: &str) {
        self.kv.insert(key.to_string(), value.to_string());
        self.persist(StorageCmd::SetKv {
            k: key.to_string(),
            v: value.to_string(),
        });
    }

    /// Clé de conversation (username de pair ou `#groupe`) épinglée en tête
    /// de la barre latérale ? Liste persistée en JSON dans la table kv.
    pub fn is_pinned(&self, conv_key: &str) -> bool {
        self.pinned_conversations().contains(&conv_key.to_string())
    }

    fn pinned_conversations(&self) -> Vec<String> {
        self.kv
            .get("pinned_conversations")
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }

    /// Épingle/désépingle une conversation (pair ou groupe) en tête de liste.
    pub fn toggle_pinned(&mut self, conv_key: &str) {
        let mut pinned = self.pinned_conversations();
        if let Some(pos) = pinned.iter().position(|c| c == conv_key) {
            pinned.remove(pos);
        } else {
            pinned.push(conv_key.to_string());
        }
        let json = serde_json::to_string(&pinned).unwrap_or_default();
        self.set_pref("pinned_conversations", &json);
        self.bump_presence();
    }

    /// Envoie une commande au thread de stockage (no-op sans stockage).
    pub(crate) fn persist(&self, cmd: StorageCmd) {
        if let Some(tx) = &self.storage {
            let _ = tx.send(cmd);
        }
    }

    /// Demande la page précédente de l'historique au thread de stockage.
    /// Renvoie `false` s'il n'y a plus rien à charger (début de l'historique
    /// atteint ou fenêtre mémoire pleine).
    pub fn request_older_messages(&self) -> bool {
        if self.messages.len() >= Self::MAX_WINDOW {
            return false;
        }
        let Some(before_rowid) = self.oldest_loaded_rowid else {
            return false;
        };
        if self.storage.is_none() {
            return false;
        }
        self.persist(StorageCmd::LoadOlder { before_rowid });
        true
    }

    /// Insère une page d'historique plus ancienne en tête de la fenêtre
    /// mémoire (résultat de [`Self::request_older_messages`]).
    pub fn prepend_older_messages(&mut self, older: Vec<ChatMessage>, oldest_rowid: Option<i64>) {
        self.oldest_loaded_rowid = oldest_rowid;
        if older.is_empty() {
            return;
        }
        self.history_cap = (self.history_cap + older.len()).min(Self::MAX_WINDOW);
        self.messages.splice(0..0, older);
        self.bump_content();
    }

    /// Flush synchrone du stockage (fermeture de l'application) : attend que
    /// toutes les commandes en file soient écrites.
    /// Lance une recherche plein texte ; le résultat arrive par `AppEvent`.
    pub fn search_history(&self, query: String) {
        self.persist(StorageCmd::Search { query });
    }

    /// Compacte la base : l'espace des conversations effacées n'est pas rendu seul.
    pub fn compact_storage(&self) {
        self.persist(StorageCmd::Compact);
    }

    /// Exporte la conversation sélectionnée en texte vers `path`.
    pub fn export_selected_conversation(&self, path: std::path::PathBuf) {
        self.persist(StorageCmd::ExportConversation {
            me: self.my_username.clone(),
            conv: self.selected_conversation.clone(),
            path,
        });
    }

    pub fn flush_storage(&self) {
        if let Some(tx) = &self.storage {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx.send(StorageCmd::Flush(ack_tx)).is_ok() {
                let _ = ack_rx.recv_timeout(std::time::Duration::from_secs(3));
            }
        }
    }

    pub(crate) fn history_cap(&self) -> usize {
        self.history_cap
    }
}
