//! Accès au cache local des médias (images et fichiers) attachés aux messages.
//!
//! Les octets sont transmis par streaming (cf. `network::media_stream`) et
//! écrits directement dans `media/<id>` ; l'historique SQLite ne
//! conserve qu'une référence légère (cf. [`crate::message::MediaAttachment`]).

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use super::AppState;

/// Délai de grâce des fichiers qu'aucun message ne référence.
///
/// Deux cas le justifient : un `.part` de réception **en cours**, que le GC
/// périodique effaçait sous le nez de son destinataire ; et le court instant
/// entre le renommage d'un média reçu et l'écriture de son message en base,
/// pendant lequel le fichier paraît orphelin alors qu'il vient d'arriver.
const ORPHAN_GRACE: Duration = Duration::from_secs(60 * 60);

/// Âge en deçà duquel le **plafond** ne sacrifie jamais une pièce jointe.
///
/// Sans ce plancher, un seul gros fichier suffisait à saturer le cache et à
/// emporter tout le reste — y compris des réceptions de la veille dont
/// l'utilisateur n'a aucune autre copie. Le plafond cède, pas les données : si
/// on ne peut pas repasser dessous sans toucher à du récent, on le dépasse et
/// on le signale. La règle d'âge, elle, reste souveraine : c'est un choix
/// explicite de l'utilisateur, pas une réaction automatique à la pression.
const CEILING_PROTECTED_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Règle de conservation du cache disque des médias.
///
/// Les deux critères se cumulent : l'âge élague en continu, le plafond sert de
/// filet. Les deux sont illimités par défaut — rien ne disparaît tant que
/// l'utilisateur n'a rien demandé.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Âge au-delà duquel une pièce jointe est purgée. `None` = illimité.
    pub max_age: Option<Duration>,
    /// Plafond du dossier `media/`, appliqué après la règle d'âge.
    /// `None` = illimité.
    pub max_bytes: Option<u64>,
}

/// Ce qu'une passe de GC a libéré (ou libérerait, en simulation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GcReport {
    pub freed_bytes: u64,
    pub freed_files: u64,
    /// Simulation : rien n'a été supprimé, c'est l'aperçu du bouton « Purger ».
    pub dry_run: bool,
    /// Le plafond n'a pas pu être tenu sans sacrifier des pièces jointes
    /// récentes. On le dit à l'utilisateur au lieu de forcer.
    pub over_ceiling: bool,
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
    /// Trop récent pour que le plafond y touche (cf. [`CEILING_PROTECTED_AGE`]).
    protected: bool,
}

/// Journalise le bilan et rend le compte rendu.
fn finish(report: GcReport, dry_run: bool) -> GcReport {
    if report.freed_files > 0 && !dry_run {
        tracing::info!(
            "GC cache : {} fichier(s) supprimé(s), {} octets libérés",
            report.freed_files,
            report.freed_bytes
        );
    }
    report
}

/// Nettoie le cache disque `media/` (au démarrage puis périodiquement, hors
/// thread UI). Les fichiers partent dans cet ordre, du moins coûteux au plus
/// coûteux pour l'utilisateur :
/// 1. transferts inachevés (`.part`) et orphelins, passé [`ORPHAN_GRACE`] ;
/// 2. pièces jointes plus vieilles que `policy.max_age` ;
/// 3. sous le plafond `policy.max_bytes`, et **uniquement** parmi ce qui a plus
///    de [`CEILING_PROTECTED_AGE`] : d'abord nos envois (l'original est
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

        // 1. Transfert inachevé ou orphelin : supprimé passé le délai de
        // grâce, qui protège une réception en cours et le bref instant où un
        // média est écrit avant que son message n'atteigne la base.
        let unreferenced = super::usage::is_part_file(name) || !referenced.contains(name);
        if unreferenced {
            if age >= ORPHAN_GRACE {
                remove(&path, meta.len(), dry_run, &mut report);
            }
            continue;
        }

        // 2. Règle d'âge : choix explicite de l'utilisateur, elle s'applique
        // quel que soit l'âge plancher du plafond.
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
            protected: age < CEILING_PROTECTED_AGE,
        });
    }

    // 3. Plafond : les envois d'abord, puis les plus anciens — et jamais une
    // pièce jointe récente. Un seul gros fichier ne doit pas emporter tout le
    // reste du cache.
    let Some(max_bytes) = policy.max_bytes else {
        return finish(report, dry_run);
    };
    kept.sort_by_key(|candidate| (!candidate.sent, candidate.modified));
    let mut total: u64 = kept.iter().map(|candidate| candidate.bytes).sum();
    for candidate in kept.iter().filter(|candidate| !candidate.protected) {
        if total <= max_bytes {
            break;
        }
        let before = report.freed_files;
        remove(&candidate.path, candidate.bytes, dry_run, &mut report);
        if report.freed_files > before {
            total -= candidate.bytes;
        }
    }
    // Toujours au-dessus : il ne reste que du récent, on préfère dépasser le
    // plafond et le dire plutôt que détruire des données fraîches.
    if total > max_bytes {
        report.over_ceiling = true;
        if !dry_run {
            tracing::warn!(
                "plafond du cache dépassé : {total} octets pour un plafond de {max_bytes}, \
                 aucune pièce jointe de plus de {} jours à supprimer",
                CEILING_PROTECTED_AGE.as_secs() / 86_400
            );
        }
    }

    finish(report, dry_run)
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
