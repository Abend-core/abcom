//! Accès au cache local des médias (images et fichiers) attachés aux messages.
//!
//! Les octets sont transmis par streaming (cf. `network::media_stream`) et
//! écrits directement dans `media/<id>` ; l'historique SQLite ne
//! conserve qu'une référence légère (cf. [`crate::message::MediaAttachment`]).

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use super::AppState;

/// Plafond par défaut du cache disque des médias (octets), repris par
/// [`RetentionPolicy::default`] tant que l'utilisateur n'a rien réglé.
pub const DEFAULT_CACHE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Délai de grâce des transferts inachevés. Un `.part` n'est référencé par
/// aucun message : sans ce sursis, le GC périodique effaçait le fichier d'une
/// réception **en cours** sous le nez de son destinataire.
const PART_GRACE: Duration = Duration::from_secs(60 * 60);

/// Règle de conservation du cache disque des médias.
///
/// Les deux critères se cumulent : l'âge élague en continu, le plafond sert de
/// filet quand un seul gros transfert suffit à saturer le disque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Âge au-delà duquel une pièce jointe est purgée. `None` = illimité.
    pub max_age: Option<Duration>,
    /// Plafond du dossier `media/`, appliqué après la règle d'âge.
    pub max_bytes: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age: None,
            max_bytes: DEFAULT_CACHE_MAX_BYTES,
        }
    }
}

/// Ce qu'une passe de GC a libéré (ou libérerait, en simulation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GcReport {
    pub freed_bytes: u64,
    pub freed_files: u64,
    /// Simulation : rien n'a été supprimé, c'est l'aperçu du bouton « Purger ».
    pub dry_run: bool,
}

impl GcReport {
    fn record(&mut self, bytes: u64) {
        self.freed_bytes += bytes;
        self.freed_files += 1;
    }
}

/// Un fichier du cache, tel que le GC le voit.
struct Candidate {
    path: PathBuf,
    modified: SystemTime,
    bytes: u64,
    /// Copie d'un fichier que nous avons envoyé : l'original est ailleurs sur
    /// la machine, c'est donc le poste le plus sûr à sacrifier.
    sent: bool,
}

/// Nettoie le cache disque `media/` (au démarrage puis périodiquement, hors
/// thread UI). Les fichiers partent dans cet ordre, du moins coûteux au plus
/// coûteux pour l'utilisateur :
/// 1. transferts inachevés (`.part`) abandonnés depuis plus de [`PART_GRACE`] ;
/// 2. orphelins — plus aucun message ne les référence ;
/// 3. pièces jointes plus vieilles que `policy.max_age` ;
/// 4. sous le plafond `policy.max_bytes` : d'abord nos envois (l'original est
///    ailleurs), puis les réceptions les plus anciennes.
///
/// Le message reste dans le fil dans tous les cas : seule la copie locale
/// disparaît, et la carte média retombe sur la carte fichier.
pub fn gc_media_dir(
    dir: PathBuf,
    referenced: HashSet<String>,
    sent: HashSet<String>,
    policy: RetentionPolicy,
    dry_run: bool,
) -> GcReport {
    let mut report = GcReport {
        dry_run,
        ..GcReport::default()
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return report;
    };
    let now = SystemTime::now();

    let mut kept: Vec<Candidate> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let age = now.duration_since(modified).unwrap_or_default();

        // 1. Transfert inachevé : on n'y touche que passé le délai de grâce,
        // sinon on casse une réception en cours.
        if super::usage::is_part_file(name) {
            if age >= PART_GRACE {
                remove(&path, meta.len(), dry_run, &mut report);
            }
            continue;
        }

        // 2. Orphelin : son message est sorti de l'historique.
        if !referenced.contains(name) {
            remove(&path, meta.len(), dry_run, &mut report);
            continue;
        }

        // 3. Règle d'âge.
        if policy.max_age.is_some_and(|max| age >= max) {
            remove(&path, meta.len(), dry_run, &mut report);
            continue;
        }

        let is_sent = sent.contains(name);
        kept.push(Candidate {
            path,
            modified,
            bytes: meta.len(),
            sent: is_sent,
        });
    }

    // 4. Plafond : les envois d'abord, puis les plus anciens.
    kept.sort_by_key(|candidate| (!candidate.sent, candidate.modified));
    let mut total: u64 = kept.iter().map(|candidate| candidate.bytes).sum();
    for candidate in &kept {
        if total <= policy.max_bytes {
            break;
        }
        let before = report.freed_files;
        remove(&candidate.path, candidate.bytes, dry_run, &mut report);
        if report.freed_files > before {
            total -= candidate.bytes;
        }
    }

    if report.freed_files > 0 && !dry_run {
        tracing::info!(
            "GC cache : {} fichier(s) supprimé(s), {} octets libérés",
            report.freed_files,
            report.freed_bytes
        );
    }
    report
}

/// Supprime un fichier et le comptabilise. En simulation, seule la
/// comptabilité a lieu — c'est ce qui alimente l'aperçu « libérerait X ».
fn remove(path: &std::path::Path, bytes: u64, dry_run: bool, report: &mut GcReport) {
    if dry_run || std::fs::remove_file(path).is_ok() {
        report.record(bytes);
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
