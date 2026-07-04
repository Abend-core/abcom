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
                // `last_seen` sert au timeout interne : sa mise à jour seule
                // ne change rien à l'affichage, pas d'invalidation de cache.
                let changed = peer.addr != addr || !peer.online;
                peer.addr = addr;
                peer.last_seen = now;
                peer.online = true;
                if changed {
                    self.bump_presence();
                }
                return;
            }
        }
        self.peers.push(Peer {
            username,
            addr,
            last_seen: now,
            online: true,
        });
        self.bump_presence();
    }

    /// Nettoie les pairs inactifs et retourne les usernames déconnectés.
    /// N'est plus appelé en production : la tâche discovery est autoritaire
    /// sur la présence (elle émet `PeerDisconnected`). Conservé comme filet
    /// de sécurité testé, réutilisable si la politique change.
    #[allow(dead_code)]
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
        if !disconnected.is_empty() {
            self.bump_presence();
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
        self.peers
            .iter()
            .any(|p| p.username == username && p.online)
    }

    /// Adresses de tous les pairs en ligne
    pub fn get_online_peers(&self) -> Vec<SocketAddr> {
        self.peers
            .iter()
            .filter(|p| p.online)
            .map(|p| p.addr)
            .collect()
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
        if let Some(rec) = self
            .peer_records
            .iter_mut()
            .find(|r| r.username == username)
        {
            rec.alias = alias;
        } else {
            use crate::message::PeerRecord;
            self.peer_records.push(PeerRecord {
                username: username.to_string(),
                alias,
            });
        }
        self.save_peer_records();
        self.bump_content();
    }

    /// Reconstruit les pairs connus depuis l'historique (hors ligne par défaut)
    pub(super) fn restore_peers_from_history(&mut self) {
        let mut known: Vec<String> = Vec::new();
        for msg in &self.messages {
            if msg.to_user.as_deref() == Some(self.my_username.as_str())
                && !known.contains(&msg.from)
            {
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
                self.peers.push(Peer {
                    username,
                    addr: dummy,
                    last_seen: 0,
                    online: false,
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/test_app_peers.rs"]
mod tests;
