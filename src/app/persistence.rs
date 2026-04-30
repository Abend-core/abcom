use std::collections::HashMap;

use crate::message::{Group, KnownNetwork, PeerRecord};
use super::AppState;

impl AppState {
    /// Écriture atomique via fichier temporaire
    fn persist_json_atomic(&self, path: &std::path::Path, json: &str) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub(super) fn load_messages(&mut self) {
        if self.history_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.history_path) {
                if let Ok(msgs) = serde_json::from_str(&content) {
                    self.messages = msgs;
                }
            }
        }
    }

    pub(super) fn load_read_counts(&mut self) {
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

    pub(super) fn load_networks(&mut self) {
        if self.networks_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.networks_path) {
                if let Ok(mut nets) = serde_json::from_str::<Vec<KnownNetwork>>(&content) {
                    for net in &mut nets {
                        if net.id.is_empty() && !net.subnet.is_empty() {
                            net.id = net.subnet.clone();
                        }
                    }
                    self.known_networks = nets;
                }
            }
        }
    }

    pub(super) fn load_peer_records(&mut self) {
        if self.peer_records_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.peer_records_path) {
                if let Ok(records) = serde_json::from_str::<Vec<PeerRecord>>(&content) {
                    self.peer_records = records;
                }
            }
        }
    }

    pub(crate) fn save_messages(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.messages) {
            if let Err(e) = self.persist_json_atomic(&self.history_path, &json) {
                eprintln!("[app] Erreur écriture messages.json: {}", e);
            }
        }
    }

    pub(crate) fn save_read_counts(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.read_counts) {
            if let Err(e) = self.persist_json_atomic(&self.read_counts_path, &json) {
                eprintln!("[app] Erreur écriture read_counts.json: {}", e);
            }
        }
    }

    pub fn save_groups(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.groups) {
            if let Err(e) = self.persist_json_atomic(&self.groups_path, &json) {
                eprintln!("[app] Erreur écriture groups.json: {}", e);
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

    pub fn save_peer_records(&self) {
        if let Some(parent) = self.peer_records_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.peer_records) {
            let _ = std::fs::write(&self.peer_records_path, json);
        }
    }
}
