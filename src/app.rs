use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::message::ChatMessage;
use crate::network::TCP_PORT;

#[derive(Clone, Debug)]
pub struct Peer {
    pub username: String,
    /// Adresse TCP du pair (port TCP_PORT)
    pub addr: SocketAddr,
}

pub struct AppState {
    pub my_username: String,
    pub peers: Vec<Peer>,
    pub messages: Vec<ChatMessage>,
    pub selected_peer: Option<usize>,
    pub selected_conversation: Option<String>,  // None = "Global", Some("Alice") = direct with Alice
    pub typing_users: HashMap<String, SystemTime>,  // qui tape, jusqu'à quand
    history_path: PathBuf,
}

impl AppState {
    pub fn new(username: String) -> Self {
        let history_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("abcom")
            .join("messages.json");

        let mut state = Self {
            my_username: username,
            peers: Vec::new(),
            messages: Vec::new(),
            selected_peer: None,
            selected_conversation: None,  // Starts with "Global"
            typing_users: HashMap::new(),
            history_path,
        };

        // Charge les messages historiques
        state.load_messages();
        state
    }

    fn load_messages(&mut self) {
        if self.history_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.history_path) {
                if let Ok(msgs) = serde_json::from_str::<Vec<ChatMessage>>(&content) {
                    self.messages = msgs;
                }
            }
        }
    }

    fn save_messages(&self) {
        if let Some(parent) = self.history_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.messages) {
            let _ = std::fs::write(&self.history_path, json);
        }
    }

    /// Ajoute ou met à jour un pair (adresse TCP déduite de l'IP + TCP_PORT)
    pub fn add_peer(&mut self, username: String, addr: SocketAddr) {
        let tcp_addr = SocketAddr::new(addr.ip(), TCP_PORT);
        for peer in &mut self.peers {
            if peer.username == username {
                peer.addr = tcp_addr;
                return;
            }
        }
        self.peers.push(Peer { username, addr: tcp_addr });
    }

    pub fn add_message(&mut self, msg: ChatMessage) {
        self.messages.push(msg);
        if self.messages.len() > 500 {
            self.messages.drain(0..100);
        }
        self.save_messages();
    }

    /// Get messages for the selected conversation
    pub fn get_conversation_messages(&self) -> Vec<&ChatMessage> {
        match &self.selected_conversation {
            None => {
                // Global: show all broadcast messages (to_user is None)
                self.messages.iter().filter(|m| m.to_user.is_none()).collect()
            }
            Some(username) => {
                // Direct: show messages from this user or to this user
                self.messages
                    .iter()
                    .filter(|m| {
                        (m.from == *username && m.to_user == Some(self.my_username.clone()))
                            || (m.from == self.my_username && m.to_user == Some(username.clone()))
                    })
                    .collect()
            }
        }
    }

    /// Get list of all active conversations (including Global)
    pub fn get_conversations(&self) -> Vec<String> {
        let mut convos = vec!["📢 Global".to_string()];
        for peer in &self.peers {
            convos.push(format!("🙋 {}", peer.username));
        }
        convos
    }

    pub fn set_user_typing(&mut self, username: String) {
        self.typing_users.insert(username, SystemTime::now());
    }

    pub fn clear_typing_if_old(&mut self) {
        let now = SystemTime::now();
        self.typing_users.retain(|_, time| {
            now.duration_since(*time)
                .map(|d| d.as_secs() < 3)  // garde pendant 3 secondes
                .unwrap_or(false)
        });
    }

    pub fn typing_users_list(&self) -> Vec<String> {
        self.typing_users.keys().cloned().collect()
    }

    /// Retourne l'adresse TCP du pair sélectionné
    pub fn selected_peer_addr(&self) -> Option<SocketAddr> {
        self.selected_peer
            .and_then(|i| self.peers.get(i))
            .map(|p| p.addr)
    }
}
