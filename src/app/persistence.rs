use std::collections::HashMap;

use crate::message::{Group, PeerRecord};
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
mod tests {
    use crate::app::AppState;
    use crate::message::{Group, PeerRecord};
    use std::path::{Path, PathBuf};

    fn tmp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("abcom_test_{}_{}", label, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn state_in(dir: &Path, username: &str) -> AppState {
        AppState::new_with_base(username, dir)
    }

    fn make_group(name: &str, owner: &str) -> Group {
        Group {
            name: name.to_string(),
            owner: owner.to_string(),
            members: vec![owner.to_string()],
            created_at: "2026-01-01 00:00:00".to_string(),
        }
    }

    // ── persist_json_atomic (via save/load groups) ─────────────────────────

    #[test]
    fn atomic_write_creates_file() {
        let dir = tmp_dir("atomic_write");
        let mut s = state_in(&dir, "alice");
        s.groups.push(make_group("Team", "alice"));
        s.save_groups();
        assert!(dir.join("groups.json").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn atomic_write_no_tmp_file_left() {
        let dir = tmp_dir("no_tmp");
        let mut s = state_in(&dir, "alice");
        s.groups.push(make_group("Dev", "alice"));
        s.save_groups();
        // Aucun fichier .tmp ne doit subsister
        let has_tmp = std::fs::read_dir(&dir).unwrap()
            .any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".tmp"));
        assert!(!has_tmp, "Fichier .tmp ne doit pas rester après écriture atomique");
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── save_groups + load_groups round-trip ───────────────────────────────

    #[test]
    fn groups_round_trip() {
        let dir = tmp_dir("groups_rt");
        // Sauvegarder
        let mut s1 = state_in(&dir, "alice");
        s1.groups.push(make_group("Alpha", "alice"));
        s1.groups.push(make_group("Beta", "alice"));
        s1.save_groups();

        // Charger dans un nouvel état
        let mut s2 = state_in(&dir, "alice");
        s2.load_groups();
        assert_eq!(s2.groups.len(), 2);
        assert!(s2.groups.iter().any(|g| g.name == "Alpha"));
        assert!(s2.groups.iter().any(|g| g.name == "Beta"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn groups_round_trip_empty() {
        let dir = tmp_dir("groups_empty");
        let s1 = state_in(&dir, "alice");
        s1.save_groups();

        let mut s2 = state_in(&dir, "alice");
        s2.load_groups();
        assert!(s2.groups.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── save_peer_records + load_peer_records round-trip ──────────────────

    #[test]
    fn peer_records_round_trip() {
        let dir = tmp_dir("peer_records_rt");
        let mut s1 = state_in(&dir, "alice");
        s1.peer_records.push(PeerRecord {
            username: "bob".to_string(),
            alias: Some("Robert".to_string()),
        });
        s1.save_peer_records();

        let mut s2 = state_in(&dir, "alice");
        s2.load_peer_records();
        assert_eq!(s2.peer_records.len(), 1);
        assert_eq!(s2.peer_records[0].alias, Some("Robert".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── save_messages + load_messages round-trip ──────────────────────────

    #[test]
    fn messages_round_trip() {
        use crate::message::ChatMessage;
        let dir = tmp_dir("messages_rt");
        let mut s1 = state_in(&dir, "alice");
        s1.messages.push(ChatMessage {
            from: "bob".to_string(),
            content: "coucou".to_string(),
            timestamp: "10:00".to_string(),
            timestamp_epoch: None,
            to_user: Some("alice".to_string()),
        });
        s1.save_messages();

        let mut s2 = state_in(&dir, "alice");
        s2.load_messages();
        assert_eq!(s2.messages.len(), 1);
        assert_eq!(s2.messages[0].content, "coucou");
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── load sur fichier inexistant ne panique pas ─────────────────────────

    #[test]
    fn load_missing_file_is_noop() {
        let dir = tmp_dir("load_missing");
        let mut s = state_in(&dir, "alice");
        // Aucun fichier sur le disque → pas de panique
        s.load_groups();
        s.load_messages();
        s.load_peer_records();
        assert!(s.groups.is_empty());
        assert!(s.messages.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
