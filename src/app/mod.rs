use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::message::{ChatMessage, Group, PeerRecord, ReactionEntry};

mod avatar;
mod groups;
mod media;
mod messages;
mod peers;
mod persistence;
mod reactions;
mod receipts;
mod transfers;
mod typing;

pub use peers::Peer;
pub use receipts::PendingMessage;

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
    history_path: PathBuf,
    read_counts_path: PathBuf,
    groups_path: PathBuf,
    peer_records_path: PathBuf,
    avatar_path: PathBuf,
    peer_avatars_path: PathBuf,
    reactions_path: PathBuf,
    media_dir: PathBuf,
}

impl AppState {
    pub fn new(username: String) -> Self {
        let base = crate::config::data_dir();

        let history_path = base.join("messages.json");
        let read_counts_path = base.join("read_counts.json");
        let groups_path = base.join("groups.json");
        let peer_records_path = base.join("peer_records.json");
        let avatar_path = base.join("avatar.png");
        let peer_avatars_path = base.join("peer_avatars.json");
        let reactions_path = base.join("reactions.json");
        let media_dir = base.join("media");

        let mut state = Self {
            my_username: username,
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
            history_path,
            read_counts_path,
            groups_path,
            peer_records_path,
            avatar_path,
            peer_avatars_path,
            reactions_path,
            media_dir,
        };

        state.load_messages();
        state.load_read_counts();
        state.load_groups();
        state.load_peer_records();
        state.load_avatar();
        state.load_peer_avatars();
        state.load_reactions();
        state.restore_peers_from_history();
        state
    }

    /// Constructeur de test avec un répertoire de données personnalisé (isolation du disque)
    #[cfg(test)]
    pub fn new_with_base(username: &str, base: &std::path::Path) -> Self {
        let history_path = base.join("messages.json");
        let read_counts_path = base.join("read_counts.json");
        let groups_path = base.join("groups.json");
        let peer_records_path = base.join("peer_records.json");
        let avatar_path = base.join("avatar.png");
        let peer_avatars_path = base.join("peer_avatars.json");
        let reactions_path = base.join("reactions.json");
        let media_dir = base.join("media");
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
            history_path,
            read_counts_path,
            groups_path,
            peer_records_path,
            avatar_path,
            peer_avatars_path,
            reactions_path,
            media_dir,
        }
    }
}
