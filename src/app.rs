use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::message::ChatMessage;
use crate::network::TCP_PORT;

#[derive(Clone, Debug)]
pub struct Peer {
    pub username: String,
    /// Adresse TCP du pair (port TCP_PORT)
    pub addr: SocketAddr,
    /// Epoch timestamp de la dernière réception d'un broadcast UDP
    pub last_seen: u64,
    /// Indique si le pair est actuellement connecté
    pub online: bool,
}

pub struct AppState {
    pub my_username: String,
    pub peers: Vec<Peer>,
    pub messages: Vec<ChatMessage>,
    pub selected_conversation: Option<String>,  // None = "Global", Some("Alice") = direct with Alice
    pub typing_users: HashMap<String, SystemTime>,  // qui tape, jusqu'à quand
    pub read_counts: HashMap<String, usize>,
    history_path: PathBuf,
    read_counts_path: PathBuf,
}

impl AppState {
    pub fn new(username: String) -> Self {
        let history_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("abcom")
            .join("messages.json");
        let read_counts_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("abcom")
            .join("read_counts.json");

        let mut state = Self {
            my_username: username,
            peers: Vec::new(),
            messages: Vec::new(),
            selected_conversation: None,  // Starts with "Global"
            typing_users: HashMap::new(),
            read_counts: HashMap::new(),
            history_path,
            read_counts_path,
        };

        // Charge les messages historiques
        state.load_messages();
        // Charge les compteurs de lecture
        state.load_read_counts();
        // Reconstruit les pairs connus depuis l'historique (hors ligne par défaut)
        state.restore_peers_from_history();
        state
    }

    /// Extrait les noms d'utilisateurs des messages privés et les ajoute comme pairs hors ligne.
    /// Cela permet d'afficher les cartes de conversation même avant la reconnexion.
    fn restore_peers_from_history(&mut self) {
        let mut known: Vec<String> = Vec::new();
        for msg in &self.messages {
            // Message reçu en privé : l'expéditeur est un pair connu
            if msg.to_user == Some(self.my_username.clone()) && !known.contains(&msg.from) {
                known.push(msg.from.clone());
            }
            // Message envoyé en privé : le destinataire est un pair connu
            if msg.from == self.my_username {
                if let Some(to) = &msg.to_user {
                    if !known.contains(to) {
                        known.push(to.clone());
                    }
                }
            }
        }
        for username in known {
            if !self.peers.iter().any(|p| p.username == username) {
                // Adresse fictive — sera mise à jour dès la reconnexion
                let dummy_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
                self.peers.push(Peer {
                    username,
                    addr: dummy_addr,
                    last_seen: 0,
                    online: false,
                });
            }
        }
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

    fn load_read_counts(&mut self) {
        if self.read_counts_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.read_counts_path) {
                if let Ok(counts) = serde_json::from_str::<HashMap<String, usize>>(&content) {
                    self.read_counts = counts;
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

    fn save_read_counts(&self) {
        if let Some(parent) = self.read_counts_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.read_counts) {
            let _ = std::fs::write(&self.read_counts_path, json);
        }
    }

    /// Ajoute ou met à jour un pair (adresse TCP déduite de l'IP + TCP_PORT)
    pub fn add_peer(&mut self, username: String, addr: SocketAddr) {
        let tcp_addr = SocketAddr::new(addr.ip(), TCP_PORT);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        for peer in &mut self.peers {
            if peer.username == username {
                peer.addr = tcp_addr;
                peer.last_seen = now;
                peer.online = true;
                return;
            }
        }
        self.peers.push(Peer { username, addr: tcp_addr, last_seen: now, online: true });
    }

    pub fn add_message(&mut self, msg: ChatMessage) {
        let incoming_from_selected = self.selected_conversation.as_ref().map(|username| {
            msg.from == *username && msg.to_user == Some(self.my_username.clone())
        }).unwrap_or(false);

        self.messages.push(msg.clone());
        if incoming_from_selected {
            self.mark_conversation_read(&msg.from);
        }

        if self.messages.len() > 500 {
            self.messages.drain(0..100);
        }
        self.save_messages();
    }

    pub fn mark_conversation_read(&mut self, peer_username: &str) {
        let count = self.messages.iter().filter(|m| {
            m.from == peer_username && m.to_user == Some(self.my_username.clone())
        }).count();
        self.read_counts.insert(peer_username.to_string(), count);
        self.save_read_counts();
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

    pub fn unread_count(&self, peer_username: &str) -> usize {
        if self.selected_conversation.as_ref() == Some(&peer_username.to_string()) {
            return 0;
        }

        let total_inbound = self.messages
            .iter()
            .filter(|m| {
                m.from == peer_username
                    && m.to_user == Some(self.my_username.clone())
            })
            .count();

        let read = *self.read_counts.get(peer_username).unwrap_or(&0);
        total_inbound.saturating_sub(read)
    }

    pub fn clear_conversation_history(&mut self) {
        match &self.selected_conversation {
            None => {
                // Remove all global broadcast messages
                self.messages.retain(|m| m.to_user.is_some());
            }
            Some(username) => {
                self.messages.retain(|m| {
                    !((m.from == *username && m.to_user == Some(self.my_username.clone()))
                        || (m.from == self.my_username && m.to_user == Some(username.clone())))
                });
            }
        }
        self.save_messages();
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

    /// Nettoie les pairs inactifs (qui n'a pas répondu depuis timeout_secs).
    /// Retourne la liste des usernames supprimés pour que l'UI puisse les notifier.
    pub fn cleanup_inactive_peers(&mut self, timeout_secs: u64) -> Vec<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut disconnected = Vec::new();

        for peer in &mut self.peers {
            let is_active = now - peer.last_seen < timeout_secs;
            if !is_active && peer.online {
                // Marquer comme hors ligne au lieu de supprimer
                peer.online = false;
                disconnected.push(peer.username.clone());
            }
        }

        disconnected
    }

    /// Retourne l'adresse TCP du pair sélectionné (via selected_conversation)
    pub fn selected_peer_addr(&self) -> Option<SocketAddr> {
        self.selected_conversation
            .as_ref()
            .and_then(|username| self.peers.iter().find(|p| p.username == *username && p.online))
            .map(|p| p.addr)
    }

    pub fn is_peer_online(&self, username: &str) -> bool {
        self.peers.iter().any(|p| p.username == username && p.online)
    }
}
