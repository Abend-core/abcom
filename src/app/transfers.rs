use std::net::SocketAddr;

use super::{AppState, ConversationId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferTarget {
    pub username: String,
    pub addr: SocketAddr,
}

impl AppState {
    pub fn selected_transfer_targets(&self) -> Vec<TransferTarget> {
        match self.selected_conversation_id() {
            ConversationId::Broadcast => self
                .peers
                .iter()
                .filter(|peer| peer.online && !peer.addr.ip().is_unspecified())
                .map(|peer| TransferTarget {
                    username: peer.username.clone(),
                    addr: peer.addr,
                })
                .collect(),
            ConversationId::Group(group_name) => {
                let Some(group) = self.get_group(&group_name) else {
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
            ConversationId::Peer(username) => self
                .peers
                .iter()
                .find(|peer| {
                    peer.online && !peer.addr.ip().is_unspecified() && peer.username == username
                })
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
