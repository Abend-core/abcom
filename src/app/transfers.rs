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
#[path = "../tests/test_app_transfers.rs"]
mod tests;
