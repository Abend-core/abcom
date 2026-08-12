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

    fn merge(&mut self, other: Entry) {
        self.bytes += other.bytes;
        self.files += other.files;
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
    /// Transferts inachevés (`.part`) : reprises impossibles, purgés dès
    /// qu'ils dépassent le délai de grâce du GC.
    pub incomplete: Entry,
    /// Notre image de profil (`avatar.png`). Celles des pairs vivent en base,
    /// donc déjà comptées dans `database`.
    pub avatar: Entry,
    /// Journaux : un fichier par jour, jamais purgés à ce jour.
    pub logs: Entry,
    /// Fichiers de travail (collages longs), déjà purgés après 24 h.
    pub scratch: Entry,
    /// Tout le reste du dossier de données : clé d'identité, `networks.json`,
    /// rapports de plantage, restes de migration. Sans ce poste le total
    /// affiché était inférieur au poids réel du dossier.
    pub other: Entry,
}

impl Usage {
    pub fn total(&self) -> Entry {
        let mut total = Entry::default();
        for entry in [
            self.database,
            self.media_received,
            self.media_sent,
            self.incomplete,
            self.avatar,
            self.logs,
            self.scratch,
            self.other,
        ] {
            total.merge(entry);
        }
        total
    }
}

/// Un transfert média en cours s'écrit sous ce préfixe (cf.
/// `network::media_stream`) : ces fichiers ne sont ni reçus ni envoyés.
pub const PART_PREFIX: &str = ".abcom-";
/// Suffixe des mêmes fichiers de transfert.
pub const PART_SUFFIX: &str = ".part";

/// Fichier de transfert inachevé ?
pub fn is_part_file(name: &str) -> bool {
    name.starts_with(PART_PREFIX) && name.ends_with(PART_SUFFIX)
}

/// Somme récursive des fichiers sous `dir`. Un dossier absent vaut zéro, ce
/// qui est le cas normal avant le premier média reçu.
fn scan_tree(dir: &Path) -> Entry {
    let mut entry = Entry::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return entry;
    };
    for item in entries.flatten() {
        let Ok(meta) = item.metadata() else {
            continue;
        };
        if meta.is_dir() {
            entry.merge(scan_tree(&item.path()));
        } else if meta.is_file() {
            entry.add(meta.len());
        }
    }
    entry
}

/// Ventile le dossier des médias entre envois, réceptions et transferts
/// inachevés, à partir des identifiants d'envoi tirés de l'historique. Un
/// fichier inconnu de `sent_ids` est compté comme reçu : c'est le cas sûr, on
/// ne présentera jamais comme « copie superflue » un fichier dont on n'a pas
/// l'original.
fn scan_media(dir: &Path, sent_ids: &HashSet<String>) -> (Entry, Entry, Entry) {
    let (mut received, mut sent, mut incomplete) =
        (Entry::default(), Entry::default(), Entry::default());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (received, sent, incomplete);
    };
    for item in entries.flatten() {
        let Ok(meta) = item.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let name = item.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if is_part_file(name) {
            incomplete.add(meta.len());
        } else if sent_ids.contains(name) {
            sent.add(meta.len());
        } else {
            received.add(meta.len());
        }
    }
    (received, sent, incomplete)
}

/// Taille de la base et de ses fichiers annexes (`-wal`, `-shm`), qui peuvent
/// peser autant qu'elle entre deux points de contrôle SQLite.
fn scan_database(data_dir: &Path) -> Entry {
    let mut entry = Entry::default();
    for name in DATABASE_FILES {
        if let Ok(meta) = std::fs::metadata(data_dir.join(name)) {
            entry.add(meta.len());
        }
    }
    entry
}

const DATABASE_FILES: [&str; 3] = ["abcom.db", "abcom.db-wal", "abcom.db-shm"];
const AVATAR_FILE: &str = "avatar.png";

/// Tout ce que le dossier de données contient et qu'aucun autre poste ne
/// compte : clé d'identité, `networks.json`, `receipts.json`, rapport de
/// plantage, restes de migration, sous-dossiers inconnus. C'est ce qui manquait
/// pour que le total affiché soit le poids réel du dossier.
fn scan_other(data_dir: &Path, media_dir: &Path) -> Entry {
    let mut entry = Entry::default();
    let Ok(entries) = std::fs::read_dir(data_dir) else {
        return entry;
    };
    let known_dirs = [
        media_dir.to_path_buf(),
        data_dir.join("logs"),
        data_dir.join("scratch"),
    ];
    for item in entries.flatten() {
        let Ok(meta) = item.metadata() else {
            continue;
        };
        let path = item.path();
        if meta.is_dir() {
            if !known_dirs.contains(&path) {
                entry.merge(scan_tree(&path));
            }
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let name = item.file_name();
        let known_file = name
            .to_str()
            .is_some_and(|name| name == AVATAR_FILE || DATABASE_FILES.contains(&name));
        if !known_file {
            entry.add(meta.len());
        }
    }
    entry
}

/// Parcourt le stockage et rend la ventilation. Bloquant : à lancer sur un
/// thread dédié.
pub fn scan(data_dir: &Path, media_dir: &Path, sent_ids: &HashSet<String>) -> Usage {
    let (media_received, media_sent, incomplete) = scan_media(media_dir, sent_ids);
    let mut avatar = Entry::default();
    if let Ok(meta) = std::fs::metadata(data_dir.join(AVATAR_FILE)) {
        avatar.add(meta.len());
    }
    Usage {
        database: scan_database(data_dir),
        media_received,
        media_sent,
        incomplete,
        avatar,
        logs: scan_tree(&data_dir.join("logs")),
        scratch: scan_tree(&data_dir.join("scratch")),
        other: scan_other(data_dir, media_dir),
    }
}

#[cfg(test)]
#[path = "../tests/test_app_usage.rs"]
mod tests;
