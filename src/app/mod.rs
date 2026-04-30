use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::message::{ChatMessage, Group, KnownNetwork, PeerRecord};

mod groups;
mod messages;
mod network_detect;
mod peers;
mod persistence;
mod receipts;
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
    pub known_networks: Vec<KnownNetwork>,
    pub peer_records: Vec<PeerRecord>,
    pub current_subnet: Option<String>,
    pub current_network_id: Option<String>,
    history_path: PathBuf,
    read_counts_path: PathBuf,
    groups_path: PathBuf,
    networks_path: PathBuf,
    peer_records_path: PathBuf,
}

impl AppState {
    pub fn new(username: String) -> Self {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("abcom");

        let history_path = base.join("messages.json");
        let read_counts_path = base.join("read_counts.json");
        let groups_path = base.join("groups.json");
        let networks_path = base.join("networks.json");
        let peer_records_path = base.join("peer_records.json");

        let current_subnet = Self::detect_subnet();
        let current_network_id = Self::detect_ssid().or_else(|| current_subnet.clone());

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
            known_networks: Vec::new(),
            peer_records: Vec::new(),
            current_subnet,
            current_network_id,
            history_path,
            read_counts_path,
            groups_path,
            networks_path,
            peer_records_path,
        };

        state.load_messages();
        state.load_read_counts();
        state.load_groups();
        state.load_networks();
        state.load_peer_records();

        if let Some(ref id) = state.current_network_id.clone() {
            let sn = state.current_subnet.clone();
            state.ensure_network_known(id, sn.as_deref());
        }
        state.restore_peers_from_history();
        state
    }

    /// Constructeur de test avec un répertoire de données personnalisé (isolation du disque)
    #[cfg(test)]
    pub fn new_with_base(username: &str, base: &std::path::Path) -> Self {
        let history_path = base.join("messages.json");
        let read_counts_path = base.join("read_counts.json");
        let groups_path = base.join("groups.json");
        let networks_path = base.join("networks.json");
        let peer_records_path = base.join("peer_records.json");
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
            known_networks: Vec::new(),
            peer_records: Vec::new(),
            current_subnet: None,
            current_network_id: None,
            history_path,
            read_counts_path,
            groups_path,
            networks_path,
            peer_records_path,
        }
    }
}
