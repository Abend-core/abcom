use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::message::{ChatMessage, Group, PeerRecord, ReactionEntry};

mod avatar;
mod groups;
pub mod media;
mod messages;
mod peers;
mod reactions;
mod receipts;
pub mod storage;
mod transfers;
mod typing;

pub use peers::Peer;
pub use receipts::PendingMessage;
pub use storage::{LoadedState, Storage, StorageCmd};

pub struct AppState {
    pub my_username: String,
    pub peers: Vec<Peer>,
    pub messages: Vec<ChatMessage>,
    pub groups: Vec<Group>,
    pub selected_conversation: Option<String>,
    pub typing_users: HashMap<String, SystemTime>,
    pub read_counts: HashMap<String, usize>,
    pub read_receipts: HashMap<u64, HashSet<String>>,
    pub pending_messages: HashMap<u64, PendingMessage>,
    pub peer_records: Vec<PeerRecord>,
    /// Avatar local (octets PNG normalisés), `None` si non défini.
    pub my_avatar: Option<Vec<u8>>,
    /// Avatars des pairs, indexés par nom d'utilisateur.
    pub peer_avatars: HashMap<String, Vec<u8>>,
    /// Réactions emoji par message, indexées par `AppState::message_hash`.
    pub reactions: HashMap<u64, Vec<ReactionEntry>>,
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
            read_receipts: HashMap::new(),
            pending_messages: HashMap::new(),
            peer_records: loaded.peer_records,
            my_avatar: None,
            peer_avatars: loaded.peer_avatars,
            reactions: loaded.reactions,
            content_generation: 0,
            presence_generation: 0,
            storage,
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
            pending_messages: HashMap::new(),
            peer_records: Vec::new(),
            my_avatar: None,
            peer_avatars: HashMap::new(),
            reactions: HashMap::new(),
            content_generation: 0,
            presence_generation: 0,
            storage: None,
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
    pub fn prepend_older_messages(
        &mut self,
        older: Vec<ChatMessage>,
        oldest_rowid: Option<i64>,
    ) {
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
