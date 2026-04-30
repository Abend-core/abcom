use std::net::SocketAddr;

use super::AppState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferTarget {
    pub username: String,
    pub addr: SocketAddr,
}

impl AppState {
    pub fn selected_transfer_targets(&self) -> Vec<TransferTarget> {
        match &self.selected_conversation {
            None => self
                .peers
                .iter()
                .filter(|peer| peer.online)
                .map(|peer| TransferTarget {
                    username: peer.username.clone(),
                    addr: peer.addr,
                })
                .collect(),
            Some(conversation) if conversation.starts_with('#') => {
                let group_name = &conversation[1..];
                let Some(group) = self.get_group(group_name) else {
                    return Vec::new();
                };

                group
                    .members
                    .iter()
                    .filter(|member| *member != &self.my_username)
                    .filter_map(|member| {
                        self.peers
                            .iter()
                            .find(|peer| peer.online && peer.username == *member)
                            .map(|peer| TransferTarget {
                                username: peer.username.clone(),
                                addr: peer.addr,
                            })
                    })
                    .collect()
            }
            Some(username) => self
                .peers
                .iter()
                .find(|peer| peer.online && peer.username == *username)
                .map(|peer| {
                    vec![TransferTarget {
                        username: peer.username.clone(),
                        addr: peer.addr,
                    }]
                })
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{AppState, Peer};

    fn state(username: &str) -> AppState {
        let mut s = AppState::new(username.to_string());
        s.peers.clear();
        s.groups.clear();
        s.messages.clear();
        s.read_counts.clear();
        s
    }

    fn peer(name: &str, addr: &str, online: bool) -> Peer {
        Peer {
            username: name.to_string(),
            addr: addr.parse().unwrap(),
            last_seen: 0,
            online,
        }
    }

    #[test]
    fn selected_transfer_targets_returns_selected_peer() {
        let mut s = state("alice");
        s.selected_conversation = Some("bob".to_string());
        s.peers.push(peer("bob", "127.0.0.1:9000", true));

        let targets = s.selected_transfer_targets();

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].username, "bob");
    }

    #[test]
    fn selected_transfer_targets_filters_group_members_to_online_peers() {
        let mut s = state("alice");
        s.peers.push(peer("bob", "127.0.0.1:9000", true));
        s.peers.push(peer("carol", "127.0.0.1:9001", false));
        s.create_group("team".to_string(), vec!["bob".to_string(), "carol".to_string()]);
        s.selected_conversation = Some("#team".to_string());

        let targets = s.selected_transfer_targets();

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].username, "bob");
    }
}