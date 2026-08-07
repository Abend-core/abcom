//! Gestion des images de profil (avatars).
//!
//! L'avatar local est conservé sur disque (`avatar.png`) tandis que ceux des
//! pairs sont mémorisés dans SQLite afin de rester visibles même
//! hors ligne ou après un redémarrage. Les octets manipulés ici sont toujours
//! des images PNG normalisées (voir `ui::avatar`).

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
        self.bump_content();
    }

    /// Retire notre avatar et supprime le fichier associé.
    pub fn clear_my_avatar(&mut self) {
        self.my_avatar = None;
        let _ = std::fs::remove_file(&self.avatar_path);
    }

    /// Enregistre (ou retire, si `png` est vide) l'avatar d'un pair, puis persiste.
    pub fn set_peer_avatar(&mut self, username: String, png: Vec<u8>) {
        let avatar = if png.is_empty() {
            self.peer_avatars.remove(&username);
            None
        } else {
            self.peer_avatars.insert(username.clone(), png.clone());
            Some(png)
        };
        self.persist(super::StorageCmd::UpsertPeerAvatar { username, avatar });
        self.bump_content();
    }

    pub(super) fn load_avatar(&mut self) {
        if let Ok(bytes) = std::fs::read(&self.avatar_path) {
            if !bytes.is_empty() {
                self.my_avatar = Some(bytes);
            }
        }
    }

    fn save_avatar(&self) {
        let Some(png) = &self.my_avatar else { return };
        if let Some(parent) = self.avatar_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&self.avatar_path, png) {
            tracing::warn!("erreur écriture avatar.png : {}", e);
        }
    }
}

#[cfg(test)]
#[path = "../tests/test_app_avatar.rs"]
mod tests;
