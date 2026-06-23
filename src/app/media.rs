//! Stockage local des médias (images et fichiers) attachés aux messages.
//!
//! Les octets reçus sur le réseau sont écrits dans `media/<id>` ; l'historique
//! `messages.json` ne conserve qu'une référence légère (cf.
//! [`crate::message::MediaAttachment`]). Le `id` inclut l'extension d'origine.

use std::path::PathBuf;

use crate::message::MediaAttachment;

use super::AppState;

impl AppState {
    /// Chemin du fichier en cache pour un média donné.
    pub fn media_path(&self, id: &str) -> PathBuf {
        self.media_dir.join(id)
    }

    /// Écrit sur disque les octets d'un média (si présents). Sans effet si le
    /// média ne porte pas de données (déjà en cache).
    pub fn store_media(&self, attachment: &MediaAttachment) {
        let Some(data) = &attachment.data else { return };
        if let Err(e) = std::fs::create_dir_all(&self.media_dir) {
            eprintln!("[app] Erreur création du dossier media/: {}", e);
            return;
        }
        let path = self.media_path(&attachment.id);
        if let Err(e) = std::fs::write(&path, data) {
            eprintln!("[app] Erreur écriture média {}: {}", attachment.id, e);
        }
    }

    /// Lit les octets d'un média depuis le cache disque.
    pub fn media_bytes(&self, id: &str) -> Option<Vec<u8>> {
        std::fs::read(self.media_path(id)).ok()
    }
}

#[cfg(test)]
mod tests {
    use crate::app::AppState;
    use crate::message::{MediaAttachment, MediaKind};

    fn attachment(id: &str, data: Vec<u8>) -> MediaAttachment {
        MediaAttachment {
            id: id.to_string(),
            filename: id.to_string(),
            kind: MediaKind::File,
            size_bytes: data.len() as u64,
            width: None,
            height: None,
            data: Some(data),
        }
    }

    #[test]
    fn store_then_read_media() {
        let dir = std::env::temp_dir().join(format!("abcom_media_{}", std::process::id()));
        let s = AppState::new_with_base("alice", &dir);
        s.store_media(&attachment("x.bin", vec![7, 8, 9]));
        assert_eq!(s.media_bytes("x.bin"), Some(vec![7, 8, 9]));
        assert!(s.media_bytes("absent.bin").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn store_without_data_is_noop() {
        let dir = std::env::temp_dir().join(format!("abcom_media_noop_{}", std::process::id()));
        let s = AppState::new_with_base("alice", &dir);
        let mut att = attachment("y.bin", vec![1]);
        att.data = None;
        s.store_media(&att);
        assert!(s.media_bytes("y.bin").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
