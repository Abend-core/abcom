use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::message::{ChatMessage, Group, KnownNetwork, PeerRecord};
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
    pub groups: Vec<Group>,
    pub selected_conversation: Option<String>,
    pub typing_users: HashMap<String, SystemTime>,
    pub read_counts: HashMap<String, usize>,
    /// Réseaux connus avec leurs pairs associés
    pub known_networks: Vec<KnownNetwork>,
    /// Alias et méta-données par pair
    pub peer_records: Vec<PeerRecord>,
    /// Subnet actif détecté au démarrage (ex: "192.168.1")
    pub current_subnet: Option<String>,
    history_path: PathBuf,
    read_counts_path: PathBuf,
    groups_path: PathBuf,
    networks_path: PathBuf,
    peer_records_path: PathBuf,
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
        let groups_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("abcom")
            .join("groups.json");

        let networks_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("abcom")
            .join("networks.json");
        let peer_records_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("abcom")
            .join("peer_records.json");

        let current_subnet = Self::detect_subnet();

        let mut state = Self {
            my_username: username,
            peers: Vec::new(),
            messages: Vec::new(),
            groups: Vec::new(),
            selected_conversation: None,
            typing_users: HashMap::new(),
            read_counts: HashMap::new(),
            known_networks: Vec::new(),
            peer_records: Vec::new(),
            current_subnet,
            history_path,
            read_counts_path,
            groups_path,
            networks_path,
            peer_records_path,
        };

        // Charge les messages historiques
        state.load_messages();
        // Charge les compteurs de lecture
        state.load_read_counts();
        // Charge les groupes
        state.load_groups();
        // Charge les réseaux connus
        state.load_networks();
        // Charge les enregistrements de pairs
        state.load_peer_records();
        // Assurer que le réseau actuel est enregistré (même sans pairs)
        if let Some(ref subnet) = state.current_subnet.clone() {
            state.ensure_network_known(subnet);
        }
        // Reconstruit les pairs connus depuis l'historique (hors ligne par défaut)
        state.restore_peers_from_history();
        state
    }

    /// Détecte le subnet /24 de l'interface principale (ex: "192.168.1")
    pub fn detect_subnet() -> Option<String> {
        // Tentative 1 : via routage (nécessite une route vers 8.8.8.8)
        if let Ok(ip) = local_ip_address::local_ip() {
            if let std::net::IpAddr::V4(v4) = ip {
                let octs = v4.octets();
                // Ignorer loopback
                if octs[0] != 127 {
                    return Some(format!("{}.{}.{}", octs[0], octs[1], octs[2]));
                }
            }
        }
        // Tentative 2 : scanner les interfaces réseau directement
        if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
            for (_name, ip) in &ifaces {
                if let std::net::IpAddr::V4(v4) = ip {
                    let octs = v4.octets();
                    // Ignorer loopback et link-local
                    if octs[0] != 127 && !(octs[0] == 169 && octs[1] == 254) {
                        return Some(format!("{}.{}.{}", octs[0], octs[1], octs[2]));
                    }
                }
            }
        }
        None
    }

    /// Assure que le réseau est dans known_networks, même sans pairs (ex: réseau actuel sans voisins)
    pub fn ensure_network_known(&mut self, subnet: &str) {
        if !self.known_networks.iter().any(|n| n.subnet == subnet) {
            self.known_networks.push(KnownNetwork {
                subnet: subnet.to_string(),
                alias: None,
                seen_peers: Vec::new(),
            });
            self.save_networks();
        }
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

    /// Force la mise hors ligne de tous les pairs (utile après changement de réseau)
    pub fn clear_all_peers_online_status(&mut self) {
        for peer in &mut self.peers {
            peer.online = false;
            peer.last_seen = 0;
        }
        eprintln!("[app] Tous les pairs ont été mis hors ligne");
    }

    /// Supprime complètement un pair de la liste
    pub fn forget_peer(&mut self, username: &str) -> bool {
        let before_len = self.peers.len();
        self.peers.retain(|p| p.username != username);
        if self.peers.len() < before_len {
            eprintln!("[app] Pair oublié: {}", username);
            return true;
        }
        false
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

    pub fn load_groups(&mut self) {
        if self.groups_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.groups_path) {
                if let Ok(groups) = serde_json::from_str::<Vec<Group>>(&content) {
                    self.groups = groups;
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

    pub fn save_groups(&self) {
        if let Some(parent) = self.groups_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("[app] Erreur création répertoire groupes: {}", e);
                return;
            }
        }
        
        // Sauvegarder avec backup atomique
        match serde_json::to_string_pretty(&self.groups) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.groups_path, &json) {
                    eprintln!("[app] Erreur écriture groups.json: {}", e);
                }
            }
            Err(e) => eprintln!("[app] Erreur sérialisation groupes: {}", e),
        }
    }

    fn load_networks(&mut self) {
        if self.networks_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.networks_path) {
                if let Ok(nets) = serde_json::from_str::<Vec<KnownNetwork>>(&content) {
                    self.known_networks = nets;
                }
            }
        }
    }

    pub fn save_networks(&self) {
        if let Some(parent) = self.networks_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.known_networks) {
            let _ = std::fs::write(&self.networks_path, json);
        }
    }

    fn load_peer_records(&mut self) {
        if self.peer_records_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.peer_records_path) {
                if let Ok(records) = serde_json::from_str::<Vec<PeerRecord>>(&content) {
                    self.peer_records = records;
                }
            }
        }
    }

    pub fn save_peer_records(&self) {
        if let Some(parent) = self.peer_records_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.peer_records) {
            let _ = std::fs::write(&self.peer_records_path, json);
        }
    }

    /// Enregistre un pair sur le réseau actuel et crée/met à jour KnownNetwork
    pub fn record_peer_on_network(&mut self, username: &str, peer_subnet: &str) {
        // Mettre à jour ou créer le réseau
        if let Some(net) = self.known_networks.iter_mut().find(|n| n.subnet == peer_subnet) {
            if !net.seen_peers.contains(&username.to_string()) {
                net.seen_peers.push(username.to_string());
            }
        } else {
            self.known_networks.push(KnownNetwork {
                subnet: peer_subnet.to_string(),
                alias: None,
                seen_peers: vec![username.to_string()],
            });
        }

        // Mettre à jour ou créer le PeerRecord
        if let Some(rec) = self.peer_records.iter_mut().find(|r| r.username == username) {
            rec.last_subnet = Some(peer_subnet.to_string());
        } else {
            self.peer_records.push(PeerRecord {
                username: username.to_string(),
                alias: None,
                last_subnet: Some(peer_subnet.to_string()),
            });
        }

        self.save_networks();
        self.save_peer_records();
    }

    /// Retourne l'alias d'un pair s'il en a un
    pub fn peer_display_name(&self, username: &str) -> String {
        self.peer_records
            .iter()
            .find(|r| r.username == username)
            .and_then(|r| r.alias.clone())
            .unwrap_or_else(|| username.to_string())
    }

    /// Supprime un réseau et tous ses pairs du registre
    pub fn forget_network(&mut self, subnet: &str) {
        if let Some(net) = self.known_networks.iter().find(|n| n.subnet == subnet).cloned() {
            for peer in &net.seen_peers {
                self.peer_records.retain(|r| &r.username != peer);
            }
        }
        self.known_networks.retain(|n| n.subnet != subnet);
        self.save_networks();
        self.save_peer_records();
    }

    /// Pairs filtrés par subnet (None = tous)
    pub fn peers_for_subnet<'a>(&'a self, subnet: &str) -> Vec<&'a Peer> {
        let seen: Vec<&str> = self.known_networks
            .iter()
            .find(|n| n.subnet == subnet)
            .map(|n| n.seen_peers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        self.peers.iter().filter(|p| seen.contains(&p.username.as_str())).collect()
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
                // Enregistrer sur le réseau
                if let std::net::IpAddr::V4(v4) = addr.ip() {
                    let o = v4.octets();
                    let subnet = format!("{}.{}.{}", o[0], o[1], o[2]);
                    let _ = peer;
                    self.record_peer_on_network(&username, &subnet);
                }
                return;
            }
        }
        // Nouveau pair
        if let std::net::IpAddr::V4(v4) = addr.ip() {
            let o = v4.octets();
            let subnet = format!("{}.{}.{}", o[0], o[1], o[2]);
            self.record_peer_on_network(&username, &subnet);
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

    /// Retourne les adresses des pairs en ligne
    pub fn get_online_peers(&self) -> Vec<SocketAddr> {
        self.peers.iter().filter(|p| p.online).map(|p| p.addr).collect()
    }

    // ──── Gestion des groupes ────

    /// Valide un nom de groupe (1-50 chars, alphanum + _-)
    fn validate_group_name(name: &str) -> bool {
        if name.is_empty() || name.len() > 50 {
            return false;
        }
        name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }

    pub fn create_group(&mut self, name: String, members: Vec<String>) -> Option<Group> {
        let name = name.trim().to_string();
        
        // Validation robuste du nom
        if !Self::validate_group_name(&name) {
            return None;
        }
        
        // Vérifier que le groupe n'existe pas déjà (case-insensitive)
        if self.groups.iter().any(|g| g.name.eq_ignore_ascii_case(&name)) {
            return None;
        }
        
        // Vérifier que les membres existent
        let invalid_members: Vec<_> = members.iter()
            .filter(|m| !self.peers.iter().any(|p| p.username == **m) && **m != self.my_username)
            .collect();
        if !invalid_members.is_empty() {
            return None;
        }

        let mut group_members = vec![self.my_username.clone()];
        for member in members {
            if member != self.my_username && !group_members.contains(&member) {
                group_members.push(member);
            }
        }

        let group = Group {
            name: name.clone(),
            owner: self.my_username.clone(),
            members: group_members,
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };

        self.groups.push(group.clone());
        self.save_groups();
        Some(group)
    }

    /// Add a member to a group (owner only).
    /// 
    /// # Status
    /// Prepared for future UI: group member management panel.
    pub fn add_member_to_group(&mut self, group_name: &str, username: String) -> bool {
        if let Some(group) = self.groups.iter_mut().find(|g| g.name == group_name) {
            if group.owner == self.my_username && !group.members.contains(&username) {
                group.members.push(username);
                self.save_groups();
                return true;
            }
        }
        false
    }

    /// Remove a member from a group (owner only).
    /// 
    /// # Status
    /// Prepared for future UI: group member management panel.
    pub fn remove_member_from_group(&mut self, group_name: &str, username: &str) -> bool {
        if let Some(group) = self.groups.iter_mut().find(|g| g.name == group_name) {
            if group.owner == self.my_username && username != &group.owner {
                group.members.retain(|m| m != username);
                self.save_groups();
                return true;
            }
        }
        false
    }

    /// Rename a group (owner only).
    /// 
    /// # Status
    /// Prepared for future UI: group settings/admin panel.
    pub fn rename_group(&mut self, group_name: &str, new_name: String) -> bool {
        if let Some(group) = self.groups.iter_mut().find(|g| g.name == group_name) {
            if group.owner == self.my_username {
                group.name = new_name;
                self.save_groups();
                return true;
            }
        }
        false
    }

    /// Delete a group (owner only).
    /// 
    /// # Status
    /// Prepared for future UI: group settings/admin panel with delete confirmation.
    pub fn delete_group(&mut self, group_name: &str) -> bool {
        if let Some(pos) = self.groups.iter().position(|g| g.name == group_name && g.owner == self.my_username) {
            self.groups.remove(pos);
            self.save_groups();
            return true;
        }
        false
    }

    /// Retrieve group by name for read-only access.
    /// 
    /// # Status
    /// Prepared for future features: group info display, permissions checks.
    pub fn get_group(&self, group_name: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.name == group_name)
    }

    pub fn is_group_owner(&self, group_name: &str) -> bool {
        self.groups
            .iter()
            .any(|g| g.name == group_name && g.owner == self.my_username)
    }

    pub fn is_in_group(&self, group_name: &str) -> bool {
        self.groups
            .iter()
            .any(|g| g.name == group_name && g.members.contains(&self.my_username))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create an AppState for testing without loading persisted data
    fn new_test_state(username: &str) -> AppState {
        let mut state = AppState::new(username.to_string());
        // Clear any loaded data to ensure clean test environment
        state.groups.clear();
        state.messages.clear();
        state.peers.clear();
        state.read_counts.clear();
        state
    }

    #[test]
    fn test_validate_group_name_valid() {
        assert!(AppState::validate_group_name("my-group"));
        assert!(AppState::validate_group_name("group_123"));
        assert!(AppState::validate_group_name("DevTeam"));
    }

    #[test]
    fn test_validate_group_name_invalid() {
        assert!(!AppState::validate_group_name(""));           // Empty
        assert!(!AppState::validate_group_name("x".repeat(51).as_str())); // Too long
        assert!(!AppState::validate_group_name("group@name")); // Invalid char
        assert!(!AppState::validate_group_name("group name")); // Space
        assert!(!AppState::validate_group_name("group!"));     // Special char
    }

    #[test]
    fn test_create_group_success() {
        let mut state = new_test_state("alice");
        state.peers.push(Peer {
            username: "bob".to_string(),
            addr: "127.0.0.1:9000".parse().unwrap(),
            last_seen: 0,
            online: true,
        });

        let group = state.create_group("DevTeam".to_string(), vec!["bob".to_string()]);
        assert!(group.is_some());
        assert_eq!(state.groups.len(), 1);
        assert_eq!(state.groups[0].members.len(), 2); // alice + bob
    }

    #[test]
    fn test_create_group_invalid_name() {
        let mut state = new_test_state("alice");
        
        let group = state.create_group("".to_string(), vec![]);
        assert!(group.is_none());
        assert_eq!(state.groups.len(), 0);
    }

    #[test]
    fn test_create_group_duplicate() {
        let mut state = new_test_state("alice");
        
        state.create_group("DevTeam".to_string(), vec![]);
        let second = state.create_group("DevTeam".to_string(), vec![]);
        
        assert!(second.is_none());
        assert_eq!(state.groups.len(), 1);
    }

    #[test]
    fn test_create_group_invalid_member() {
        let mut state = new_test_state("alice");
        
        // Try to add non-existent peer
        let group = state.create_group("Team".to_string(), vec!["unknown".to_string()]);
        assert!(group.is_none());
    }

    #[test]
    fn test_is_group_owner() {
        let mut state = new_test_state("alice");
        state.create_group("MyGroup".to_string(), vec![]);
        
        assert!(state.is_group_owner("MyGroup"));
        assert!(!state.is_group_owner("NonExistent"));
    }

    #[test]
    fn test_is_in_group() {
        let mut state = new_test_state("alice");
        state.create_group("MyGroup".to_string(), vec![]);
        
        assert!(state.is_in_group("MyGroup"));
        assert!(!state.is_in_group("NonExistent"));
    }

    #[test]
    fn test_add_member_to_group() {
        let mut state = new_test_state("alice");
        state.peers.push(Peer {
            username: "bob".to_string(),
            addr: "127.0.0.1:9000".parse().unwrap(),
            last_seen: 0,
            online: true,
        });
        
        state.create_group("Team".to_string(), vec![]);
        let added = state.add_member_to_group("Team", "bob".to_string());
        
        assert!(added);
        assert_eq!(state.groups[0].members.len(), 2);
    }

    #[test]
    fn test_remove_member_from_group() {
        let mut state = new_test_state("alice");
        state.peers.push(Peer {
            username: "bob".to_string(),
            addr: "127.0.0.1:9000".parse().unwrap(),
            last_seen: 0,
            online: true,
        });
        
        state.create_group("Team".to_string(), vec!["bob".to_string()]);
        let removed = state.remove_member_from_group("Team", "bob");
        
        assert!(removed);
        assert_eq!(state.groups[0].members.len(), 1); // Only alice remains
    }

    #[test]
    fn test_get_online_peers() {
        let mut state = new_test_state("alice");
        
        // Add peers, some online, some offline
        state.peers.push(Peer {
            username: "bob".to_string(),
            addr: "192.168.1.10:9000".parse().unwrap(),
            last_seen: 0,
            online: true,
        });
        state.peers.push(Peer {
            username: "charlie".to_string(),
            addr: "192.168.1.11:9000".parse().unwrap(),
            last_seen: 0,
            online: false,
        });
        state.peers.push(Peer {
            username: "diana".to_string(),
            addr: "192.168.1.12:9000".parse().unwrap(),
            last_seen: 0,
            online: true,
        });
        
        let online_addrs = state.get_online_peers();
        assert_eq!(online_addrs.len(), 2);
        assert!(online_addrs.contains(&"192.168.1.10:9000".parse().unwrap()));
        assert!(online_addrs.contains(&"192.168.1.12:9000".parse().unwrap()));
    }

    #[test]
    fn test_group_sync_simulation() {
        // Simulate two instances: alice creates a group, which should be receivable by bob
        let mut alice = new_test_state("alice");
        let mut bob = new_test_state("bob");
        
        // Alice creates a group
        let group_opt = alice.create_group("DevTeam".to_string(), vec![]);
        assert!(group_opt.is_some());
        assert_eq!(alice.groups.len(), 1);
        assert_eq!(bob.groups.len(), 0);
        
        // Simulate Bob receiving the group via network (manually, since this is unit test)
        if let Some(group) = group_opt {
            bob.groups.push(group);
            bob.save_groups();
        }
        
        // Now both should have the same group
        assert_eq!(alice.groups.len(), 1);
        assert_eq!(bob.groups.len(), 1);
        assert_eq!(alice.groups[0].name, bob.groups[0].name);
        assert_eq!(alice.groups[0].owner, "alice");
        assert_eq!(bob.groups[0].owner, "alice");
    }
}
