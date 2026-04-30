use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::network::TCP_PORT;

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
    /// Ajoute ou met à jour un pair (adresse TCP déduite de l'IP + TCP_PORT)
    pub fn add_peer(&mut self, username: String, addr: SocketAddr) {
        let tcp_addr = SocketAddr::new(addr.ip(), TCP_PORT);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let network_id = self.current_network_id.clone();
        for peer in &mut self.peers {
            if peer.username == username {
                peer.addr = tcp_addr;
                peer.last_seen = now;
                peer.online = true;
                let _ = peer;
                if let Some(ref id) = network_id {
                    self.record_peer_on_network(&username, id);
                }
                return;
            }
        }
        if let Some(ref id) = network_id {
            self.record_peer_on_network(&username, id);
        }
        self.peers.push(Peer { username, addr: tcp_addr, last_seen: now, online: true });
    }

    /// Force la mise hors ligne de tous les pairs (après changement de réseau)
    pub fn clear_all_peers_online_status(&mut self) {
        for peer in &mut self.peers {
            peer.online = false;
            peer.last_seen = 0;
        }
    }

    /// Supprime complètement un pair de la liste
    pub fn forget_peer(&mut self, username: &str) -> bool {
        let before = self.peers.len();
        self.peers.retain(|p| p.username != username);
        self.peers.len() < before
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

    /// Pairs filtrés par network_id
    pub fn peers_for_network<'a>(&'a self, network_id: &str) -> Vec<&'a Peer> {
        let seen: Vec<&str> = self.known_networks
            .iter()
            .find(|n| n.id == network_id)
            .map(|n| n.seen_peers.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        self.peers.iter().filter(|p| seen.contains(&p.username.as_str())).collect()
    }

    /// Enregistre un pair sur le réseau actuel
    pub fn record_peer_on_network(&mut self, username: &str, network_id: &str) {
        if let Some(net) = self.known_networks.iter_mut().find(|n| n.id == network_id) {
            if !net.seen_peers.contains(&username.to_string()) {
                net.seen_peers.push(username.to_string());
            }
        } else {
            use crate::message::KnownNetwork;
            self.known_networks.push(KnownNetwork {
                id: network_id.to_string(),
                subnet: self.current_subnet.clone().unwrap_or_default(),
                alias: None,
                seen_peers: vec![username.to_string()],
            });
        }

        if let Some(rec) = self.peer_records.iter_mut().find(|r| r.username == username) {
            rec.last_subnet = Some(network_id.to_string());
        } else {
            use crate::message::PeerRecord;
            self.peer_records.push(PeerRecord {
                username: username.to_string(),
                alias: None,
                last_subnet: Some(network_id.to_string()),
            });
        }

        self.save_networks();
        self.save_peer_records();
    }

    /// Supprime un réseau et tous ses pairs du registre
    pub fn forget_network(&mut self, network_id: &str) {
        if let Some(net) = self.known_networks.iter().find(|n| n.id == network_id).cloned() {
            for peer in &net.seen_peers {
                self.peer_records.retain(|r| &r.username != peer);
            }
        }
        self.known_networks.retain(|n| n.id != network_id);
        self.save_networks();
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
