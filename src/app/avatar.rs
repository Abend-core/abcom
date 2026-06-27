//! Gestion des images de profil (avatars).
//!
//! L'avatar local est conservé sur disque (`avatar.png`) tandis que ceux des
//! pairs sont mémorisés dans `peer_avatars.json` afin de rester visibles même
//! hors ligne ou après un redémarrage. Les octets manipulés ici sont toujours
//! des images PNG normalisées (voir `ui::avatar`).

use std::collections::HashMap;

use crate::message::AvatarAnnounce;

use super::AppState;

impl AppState {
    /// Octets PNG de l'avatar d'un utilisateur (le nôtre ou celui d'un pair).
    pub fn avatar_bytes(&self, username: &str) -> Option<Vec<u8>> {
        if username == self.my_username {
            self.my_avatar.clone()
        } else {
            self.peer_avatars.get(username).cloned()
        }
    }

    /// Construit l'annonce réseau de notre avatar, si nous en avons un.
    pub fn avatar_announce(&self) -> Option<AvatarAnnounce> {
        self.my_avatar.as_ref().map(|png| AvatarAnnounce {
            from: self.my_username.clone(),
            png: png.clone(),
        })
    }

    /// Définit notre avatar (octets PNG normalisés) puis le persiste.
    pub fn set_my_avatar(&mut self, png: Vec<u8>) {
        self.my_avatar = Some(png);
        self.save_avatar();
    }

    /// Retire notre avatar et supprime le fichier associé.
    pub fn clear_my_avatar(&mut self) {
        self.my_avatar = None;
        let _ = std::fs::remove_file(&self.avatar_path);
    }

    /// Enregistre (ou retire, si `png` est vide) l'avatar d'un pair, puis persiste.
    pub fn set_peer_avatar(&mut self, username: String, png: Vec<u8>) {
        if png.is_empty() {
            self.peer_avatars.remove(&username);
        } else {
            self.peer_avatars.insert(username, png);
        }
        self.save_peer_avatars();
    }

    pub(super) fn load_avatar(&mut self) {
        if let Ok(bytes) = std::fs::read(&self.avatar_path) {
            if !bytes.is_empty() {
                self.my_avatar = Some(bytes);
            }
        }
    }

    pub(super) fn load_peer_avatars(&mut self) {
        if let Ok(content) = std::fs::read_to_string(&self.peer_avatars_path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, Vec<u8>>>(&content) {
                self.peer_avatars = map;
            }
        }
    }

    fn save_avatar(&self) {
        let Some(png) = &self.my_avatar else { return };
        if let Some(parent) = self.avatar_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&self.avatar_path, png) {
            eprintln!("[app] Erreur écriture avatar.png: {}", e);
        }
    }

    fn save_peer_avatars(&self) {
        if let Some(parent) = self.peer_avatars_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(&self.peer_avatars) {
            if let Err(e) = std::fs::write(&self.peer_avatars_path, json) {
                eprintln!("[app] Erreur écriture peer_avatars.json: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::AppState;

    fn tmp_dir(label: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("abcom_avatar_{}_{}", label, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn my_avatar_round_trip() {
        let dir = tmp_dir("mine");
        let mut s1 = AppState::new_with_base("alice", &dir);
        s1.set_my_avatar(vec![9, 8, 7]);
        assert_eq!(s1.avatar_bytes("alice"), Some(vec![9, 8, 7]));

        let mut s2 = AppState::new_with_base("alice", &dir);
        s2.load_avatar();
        assert_eq!(s2.my_avatar, Some(vec![9, 8, 7]));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clear_my_avatar_removes_file() {
        let dir = tmp_dir("clear");
        let mut s = AppState::new_with_base("alice", &dir);
        s.set_my_avatar(vec![1, 2, 3]);
        s.clear_my_avatar();
        assert!(s.my_avatar.is_none());
        assert!(!dir.join("avatar.png").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn peer_avatar_set_and_remove() {
        let dir = tmp_dir("peer");
        let mut s = AppState::new_with_base("alice", &dir);
        s.set_peer_avatar("bob".to_string(), vec![4, 5]);
        assert_eq!(s.avatar_bytes("bob"), Some(vec![4, 5]));
        s.set_peer_avatar("bob".to_string(), Vec::new());
        assert!(s.avatar_bytes("bob").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn avatar_announce_built_from_own_avatar() {
        let dir = tmp_dir("announce");
        let mut s = AppState::new_with_base("alice", &dir);
        assert!(s.avatar_announce().is_none());
        s.set_my_avatar(vec![1]);
        let announce = s.avatar_announce().unwrap();
        assert_eq!(announce.from, "alice");
        assert_eq!(announce.png, vec![1]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
