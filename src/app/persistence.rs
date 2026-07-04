use std::collections::HashMap;
use std::path::PathBuf;

use super::AppState;
use crate::message::{ChatMessage, Group, PeerRecord, ReactionEntry};

/// Écriture atomique via fichier temporaire.
fn persist_json_atomic(path: &std::path::Path, json: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn write_json<T: serde::Serialize>(path: &std::path::Path, value: &T, label: &str) {
    match serde_json::to_string(value) {
        Ok(json) => {
            if let Err(e) = persist_json_atomic(path, &json) {
                eprintln!("[app] Erreur écriture {}: {}", label, e);
            }
        }
        Err(e) => eprintln!("[app] Erreur sérialisation {}: {}", label, e),
    }
}

/// Instantané des structures modifiées, à écrire **hors du thread UI** (les
/// données sont clonées sous verrou, la sérialisation et l'I/O se font dans
/// un thread détaché, cf. `AbcomApp::periodic_tasks`).
pub struct PersistJob {
    messages: Option<(Vec<ChatMessage>, PathBuf)>,
    read_counts: Option<(HashMap<String, usize>, PathBuf)>,
    reactions: Option<(HashMap<u64, Vec<ReactionEntry>>, PathBuf)>,
}

impl PersistJob {
    pub fn is_empty(&self) -> bool {
        self.messages.is_none() && self.read_counts.is_none() && self.reactions.is_none()
    }

    /// Écrit l'instantané sur disque (appelé depuis un thread détaché).
    pub fn write(self) {
        if let Some((messages, path)) = self.messages {
            write_json(&path, &messages, "messages.json");
        }
        if let Some((counts, path)) = self.read_counts {
            write_json(&path, &counts, "read_counts.json");
        }
        if let Some((reactions, path)) = self.reactions {
            write_json(&path, &reactions, "reactions.json");
        }
    }
}

impl AppState {
    /// Prélève un instantané des structures marquées dirty et efface les
    /// marqueurs. Renvoyé au thread d'écriture ; `is_empty()` si rien à faire.
    pub fn take_persist_job(&mut self) -> PersistJob {
        let job = PersistJob {
            messages: self
                .dirty
                .messages
                .then(|| (self.messages.clone(), self.history_path.clone())),
            read_counts: self
                .dirty
                .read_counts
                .then(|| (self.read_counts.clone(), self.read_counts_path.clone())),
            reactions: self
                .dirty
                .reactions
                .then(|| (self.reactions.clone(), self.reactions_path.clone())),
        };
        self.dirty = super::DirtyFlags::default();
        job
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

    pub(super) fn load_reactions(&mut self) {
        if self.reactions_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&self.reactions_path) {
                if let Ok(reactions) =
                    serde_json::from_str::<HashMap<u64, Vec<ReactionEntry>>>(&content)
                {
                    self.reactions = reactions;
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

    /// Écriture synchrone immédiate (sortie de l'application et tests) ; le
    /// chemin nominal passe par [`AppState::take_persist_job`] (débouncé).
    pub(crate) fn save_messages(&self) {
        write_json(&self.history_path, &self.messages, "messages.json");
    }

    /// Écriture synchrone immédiate (sortie de l'application et tests).
    pub(crate) fn save_read_counts(&self) {
        write_json(&self.read_counts_path, &self.read_counts, "read_counts.json");
    }

    pub fn save_groups(&self) {
        write_json(&self.groups_path, &self.groups, "groups.json");
    }

    /// Écriture synchrone immédiate (sortie de l'application et tests).
    pub(crate) fn save_reactions(&self) {
        write_json(&self.reactions_path, &self.reactions, "reactions.json");
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

#[cfg(test)]
#[path = "../tests/test_app_persistence.rs"]
mod tests;
