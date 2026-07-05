//! Accès au cache local des médias (images et fichiers) attachés aux messages.
//!
//! Les octets sont transmis par streaming (cf. `network::media_stream`) et
//! écrits directement dans `media/<id>` ; l'historique `messages.json` ne
//! conserve qu'une référence légère (cf. [`crate::message::MediaAttachment`]).

use std::collections::HashSet;
use std::path::PathBuf;

use super::AppState;

/// Plafond du cache disque des médias reçus (octets). Au-delà, les fichiers
/// les plus anciens sont supprimés (le fil retombe alors sur la carte
/// fichier : le message reste, seul l'aperçu local disparaît).
const MEDIA_CACHE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Nettoie le cache disque `media/` (appelé au démarrage, hors thread UI) :
/// 1. supprime les fichiers orphelins — non référencés par l'historique
///    (leurs messages sont sortis du ring-buffer) : sans cela le dossier
///    grossit indéfiniment ;
/// 2. applique le plafond [`MEDIA_CACHE_MAX_BYTES`] en supprimant les plus
///    anciens (mtime) en premier.
pub fn gc_media_dir(dir: PathBuf, referenced: HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };

    let mut kept: Vec<(PathBuf, std::time::SystemTime, u64)> = Vec::new();
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !referenced.contains(name) {
            if std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            kept.push((path, mtime, meta.len()));
        }
    }

    kept.sort_by_key(|(_, mtime, _)| *mtime);
    let mut total: u64 = kept.iter().map(|(_, _, len)| len).sum();
    let mut index = 0;
    while total > MEDIA_CACHE_MAX_BYTES && index < kept.len() {
        let (path, _, len) = &kept[index];
        if std::fs::remove_file(path).is_ok() {
            total -= len;
            removed += 1;
        }
        index += 1;
    }

    if removed > 0 {
        eprintln!("[media] GC cache : {removed} fichier(s) supprimé(s)");
    }
}

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
        self.persist(super::StorageCmd::DeleteMessageByMediaId(
            media_id.to_string(),
        ));
        self.bump_content();
    }
}

#[cfg(test)]
#[path = "../tests/test_app_media.rs"]
mod tests;
