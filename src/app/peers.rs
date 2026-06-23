use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use super::AppState;

/// Représentation d'un pair LAN
#[derive(Clone, Debug)]
pub struct Peer {
    pub username: String,
    pub addr: SocketAddr,
    pub last_seen: u64,
    pub online: bool,
}

impl AppState {
    /// Ajoute ou met à jour un pair. `addr` est l'adresse TCP de chat complète
    /// (IP source + port annoncé lors de la découverte).
    pub fn add_peer(&mut self, username: String, addr: SocketAddr) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for peer in &mut self.peers {
            if peer.username == username {
                peer.addr = addr;
                peer.last_seen = now;
                peer.online = true;
                return;
            }
        }
        self.peers.push(Peer { username, addr, last_seen: now, online: true });
    }

    /// Nettoie les pairs inactifs et retourne les usernames déconnectés
    pub fn cleanup_inactive_peers(&mut self, timeout_secs: u64) -> Vec<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut disconnected = Vec::new();
        for peer in &mut self.peers {
            if now - peer.last_seen >= timeout_secs && peer.online {
                peer.online = false;
                disconnected.push(peer.username.clone());
            }
        }
        disconnected
    }

    /// Adresse TCP du pair sélectionné (via selected_conversation)
    pub fn selected_peer_addr(&self) -> Option<SocketAddr> {
        self.selected_conversation
            .as_ref()
            .and_then(|u| self.peers.iter().find(|p| p.username == *u && p.online))
            .map(|p| p.addr)
    }

    pub fn is_peer_online(&self, username: &str) -> bool {
        self.peers.iter().any(|p| p.username == username && p.online)
    }

    /// Adresses de tous les pairs en ligne
    pub fn get_online_peers(&self) -> Vec<SocketAddr> {
        self.peers.iter().filter(|p| p.online).map(|p| p.addr).collect()
    }

    /// Alias d'un pair s'il en a un, sinon son username
    pub fn peer_display_name(&self, username: &str) -> String {
        self.peer_records
            .iter()
            .find(|r| r.username == username)
            .and_then(|r| r.alias.clone())
            .unwrap_or_else(|| username.to_string())
    }

    /// Définit (ou retire, si `None`) l'alias d'un pair, puis persiste.
    pub fn set_peer_alias(&mut self, username: &str, alias: Option<String>) {
        if let Some(rec) = self.peer_records.iter_mut().find(|r| r.username == username) {
            rec.alias = alias;
        } else {
            use crate::message::PeerRecord;
            self.peer_records.push(PeerRecord {
                username: username.to_string(),
                alias,
            });
        }
        self.save_peer_records();
    }

    /// Reconstruit les pairs connus depuis l'historique (hors ligne par défaut)
    pub(super) fn restore_peers_from_history(&mut self) {
        let mut known: Vec<String> = Vec::new();
        for msg in &self.messages {
            if msg.to_user == Some(self.my_username.clone()) && !known.contains(&msg.from) {
                known.push(msg.from.clone());
            }
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
                let dummy: SocketAddr = "0.0.0.0:0".parse().unwrap();
                self.peers.push(Peer { username, addr: dummy, last_seen: 0, online: false });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use crate::app::{AppState, Peer};

    fn state(username: &str) -> AppState {
        let mut s = AppState::new(username.to_string());
        s.peers.clear();
        s.messages.clear();
        s.groups.clear();
        s.read_counts.clear();
        s.peer_records.clear();
        s
    }

    fn peer(name: &str, ip: &str, online: bool) -> Peer {
        let addr: SocketAddr = format!("{}:9000", ip).parse().unwrap();
        Peer { username: name.to_string(), addr, last_seen: 0, online }
    }

    #[test]
    fn test_add_peer_new() {
        let mut s = state("alice");
        // L'adresse fournie (IP + port de chat annoncé) est stockée telle quelle
        let addr: SocketAddr = "192.168.1.5:9010".parse().unwrap();
        s.add_peer("bob".to_string(), addr);
        assert_eq!(s.peers.len(), 1);
        assert_eq!(s.peers[0].username, "bob");
        assert_eq!(s.peers[0].addr, addr);
        assert!(s.peers[0].online);
    }

    #[test]
    fn test_add_peer_updates_existing() {
        let mut s = state("alice");
        let a1: SocketAddr = "192.168.1.5:1234".parse().unwrap();
        let a2: SocketAddr = "192.168.1.6:1234".parse().unwrap();
        s.add_peer("bob".to_string(), a1);
        s.add_peer("bob".to_string(), a2);
        assert_eq!(s.peers.len(), 1, "no duplicate");
        assert_eq!(s.peers[0].addr.ip().to_string(), "192.168.1.6");
    }

    #[test]
    fn test_cleanup_inactive_peers() {
        let mut s = state("alice");
        // last_seen = 0 → very old, online = true
        s.peers.push(peer("bob", "192.168.1.5", true));
        let disc = s.cleanup_inactive_peers(1);
        assert_eq!(disc, vec!["bob".to_string()]);
        assert!(!s.peers[0].online);
    }

    #[test]
    fn test_cleanup_inactive_peers_recent_stays_online() {
        let mut s = state("alice");
        // last_seen very recent: use std epoch + current time
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        s.peers.push(Peer {
            username: "bob".to_string(),
            addr: "192.168.1.5:9000".parse().unwrap(),
            last_seen: now,
            online: true,
        });
        let disc = s.cleanup_inactive_peers(30);
        assert!(disc.is_empty());
        assert!(s.peers[0].online);
    }

    #[test]
    fn test_is_peer_online() {
        let mut s = state("alice");
        s.peers.push(peer("bob", "192.168.1.5", true));
        s.peers.push(peer("charlie", "192.168.1.6", false));
        assert!(s.is_peer_online("bob"));
        assert!(!s.is_peer_online("charlie"));
        assert!(!s.is_peer_online("nobody"));
    }

    #[test]
    fn test_get_online_peers() {
        let mut s = state("alice");
        s.peers.push(peer("bob", "192.168.1.5", true));
        s.peers.push(peer("charlie", "192.168.1.6", false));
        let online = s.get_online_peers();
        assert_eq!(online.len(), 1);
        assert_eq!(online[0].ip().to_string(), "192.168.1.5");
    }

    #[test]
    fn test_selected_peer_addr_none_when_no_selection() {
        let s = state("alice");
        assert!(s.selected_peer_addr().is_none());
    }

    #[test]
    fn test_selected_peer_addr_returns_addr() {
        let mut s = state("alice");
        s.peers.push(peer("bob", "192.168.1.5", true));
        s.selected_conversation = Some("bob".to_string());
        assert!(s.selected_peer_addr().is_some());
        assert_eq!(s.selected_peer_addr().unwrap().ip().to_string(), "192.168.1.5");
    }

    #[test]
    fn test_selected_peer_addr_none_when_offline() {
        let mut s = state("alice");
        s.peers.push(peer("bob", "192.168.1.5", false));
        s.selected_conversation = Some("bob".to_string());
        assert!(s.selected_peer_addr().is_none());
    }

    #[test]
    fn test_peer_display_name_no_alias() {
        let s = state("alice");
        assert_eq!(s.peer_display_name("bob"), "bob");
    }

    #[test]
    fn test_peer_display_name_with_alias() {
        use crate::message::PeerRecord;
        let mut s = state("alice");
        s.peer_records.push(PeerRecord {
            username: "bob".to_string(),
            alias: Some("Robert".to_string()),
        });
        assert_eq!(s.peer_display_name("bob"), "Robert");
    }

    #[test]
    fn test_set_peer_alias_then_clear() {
        // new_with_base isole les écritures disque dans un répertoire temporaire
        let dir = std::env::temp_dir().join(format!("abcom_alias_{}", std::process::id()));
        let mut s = AppState::new_with_base("alice", &dir);
        s.set_peer_alias("bob", Some("Robert".to_string()));
        assert_eq!(s.peer_display_name("bob"), "Robert");
        s.set_peer_alias("bob", None);
        assert_eq!(s.peer_display_name("bob"), "bob");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
