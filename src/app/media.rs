//! Accès au cache local des médias (images et fichiers) attachés aux messages.
//!
//! Les octets sont transmis par streaming (cf. `network::media_stream`) et
//! écrits directement dans `media/<id>` ; l'historique `messages.json` ne
//! conserve qu'une référence légère (cf. [`crate::message::MediaAttachment`]).

use std::path::PathBuf;

use super::AppState;

impl AppState {
    /// Chemin du fichier en cache pour un média donné.
    pub fn media_path(&self, id: &str) -> PathBuf {
        self.media_dir.join(id)
    }

    /// Lit les octets d'un média depuis le cache disque.
    pub fn media_bytes(&self, id: &str) -> Option<Vec<u8>> {
        std::fs::read(self.media_path(id)).ok()
    }

    /// Retire de l'historique le message portant ce média et supprime son
    /// fichier en cache (réception interrompue).
    pub fn remove_media_message(&mut self, media_id: &str) {
        let _ = std::fs::remove_file(self.media_path(media_id));
        self.messages
            .retain(|m| m.media.as_ref().is_none_or(|x| x.id != media_id));
        self.save_messages();
    }
}

#[cfg(test)]
#[path = "../tests/test_app_media.rs"]
mod tests;
