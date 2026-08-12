//! Occupation disque de l'application, ventilée par poste (onglet Stockage).
//!
//! Le calcul parcourt des dossiers entiers : il ne doit jamais tourner sur le
//! thread de rendu. [`scan`] est volontairement une fonction libre, sans accès
//! à l'état partagé — l'appelant la lance sur un thread dédié et n'a qu'à lui
//! passer des chemins et la liste des identifiants d'envois.

use std::collections::HashSet;
use std::path::Path;

/// Taille et nombre de fichiers d'un poste de stockage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub bytes: u64,
    pub files: u64,
}

impl Entry {
    fn add(&mut self, bytes: u64) {
        self.bytes += bytes;
        self.files += 1;
    }
}

/// Ventilation complète, telle qu'affichée dans Paramètres → Stockage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Base SQLite : historique des messages, réactions, accusés. Inclut les
    /// fichiers annexes `-wal` et `-shm`, qui appartiennent à la base.
    pub database: Entry,
    /// Médias reçus des pairs — les supprimer perd les fichiers.
    pub media_received: Entry,
    /// Médias envoyés : copies de fichiers dont l'utilisateur garde
    /// l'original ailleurs, donc les plus sûrs à purger.
    pub media_sent: Entry,
    /// Notre image de profil (`avatar.png`). Celles des pairs vivent en base,
    /// donc déjà comptées dans `database`.
    pub avatar: Entry,
    /// Journaux : un fichier par jour, jamais purgés à ce jour.
    pub logs: Entry,
    /// Fichiers de travail (collages longs), déjà purgés après 24 h.
    pub scratch: Entry,
}

impl Usage {
    pub fn total(&self) -> Entry {
        Entry {
            bytes: self.database.bytes
                + self.media_received.bytes
                + self.media_sent.bytes
                + self.avatar.bytes
                + self.logs.bytes
                + self.scratch.bytes,
            files: self.database.files
                + self.media_received.files
                + self.media_sent.files
                + self.avatar.files
                + self.logs.files
                + self.scratch.files,
        }
    }
}

/// Somme les fichiers directement contenus dans `dir` (sans récursion : aucun
/// de ces dossiers n'a de sous-dossier). Un dossier absent vaut zéro, ce qui
/// est le cas normal avant le premier média reçu.
fn scan_dir(dir: &Path) -> Entry {
    let mut entry = Entry::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return entry;
    };
    for item in entries.flatten() {
        if let Ok(meta) = item.metadata() {
            if meta.is_file() {
                entry.add(meta.len());
            }
        }
    }
    entry
}

/// Ventile le dossier des médias entre envois et réceptions à partir des
/// identifiants d'envoi tirés de l'historique. Un fichier inconnu de
/// `sent_ids` est compté comme reçu : c'est le cas sûr, on ne présentera
/// jamais comme « copie superflue » un fichier dont on n'a pas l'original.
fn scan_media(dir: &Path, sent_ids: &HashSet<String>) -> (Entry, Entry) {
    let (mut received, mut sent) = (Entry::default(), Entry::default());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (received, sent);
    };
    for item in entries.flatten() {
        let Ok(meta) = item.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let is_sent = item
            .file_name()
            .to_str()
            .is_some_and(|name| sent_ids.contains(name));
        if is_sent {
            sent.add(meta.len());
        } else {
            received.add(meta.len());
        }
    }
    (received, sent)
}

/// Taille de la base et de ses fichiers annexes (`-wal`, `-shm`), qui peuvent
/// peser autant qu'elle entre deux points de contrôle SQLite.
fn scan_database(data_dir: &Path) -> Entry {
    let mut entry = Entry::default();
    for name in ["abcom.db", "abcom.db-wal", "abcom.db-shm"] {
        if let Ok(meta) = std::fs::metadata(data_dir.join(name)) {
            entry.add(meta.len());
        }
    }
    entry
}

/// Parcourt le stockage et rend la ventilation. Bloquant : à lancer sur un
/// thread dédié.
pub fn scan(data_dir: &Path, media_dir: &Path, sent_ids: &HashSet<String>) -> Usage {
    let (media_received, media_sent) = scan_media(media_dir, sent_ids);
    let mut avatar = Entry::default();
    if let Ok(meta) = std::fs::metadata(data_dir.join("avatar.png")) {
        avatar.add(meta.len());
    }
    Usage {
        database: scan_database(data_dir),
        media_received,
        media_sent,
        avatar,
        logs: scan_dir(&data_dir.join("logs")),
        scratch: scan_dir(&data_dir.join("scratch")),
    }
}

#[cfg(test)]
#[path = "../tests/test_app_usage.rs"]
mod tests;
