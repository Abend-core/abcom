use std::net::SocketAddr;

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
}

impl AppState {
    pub fn new(username: String) -> Self {
        Self {
            my_username: username,
            peers: Vec::new(),
            messages: Vec::new(),
            selected_peer: None,
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
    }

    /// Retourne l'adresse TCP du pair sélectionné
    pub fn selected_peer_addr(&self) -> Option<SocketAddr> {
        self.selected_peer
            .and_then(|i| self.peers.get(i))
            .map(|p| p.addr)
    }
}
