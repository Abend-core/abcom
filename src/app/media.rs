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
/// Deux cas le justifient : un `.part` de réception **en cours**, qu'une purge
/// déclenchée au mauvais moment effacerait sous le nez de son destinataire ; et
/// le court instant entre le renommage d'un média reçu et l'écriture de son
/// message en base, pendant lequel le fichier paraît orphelin alors qu'il vient
/// d'arriver.
const ORPHAN_GRACE: Duration = Duration::from_secs(60 * 60);

/// Ce qu'une purge doit supprimer.
///
/// Il n'existe **aucune purge automatique** : rien n'est jamais supprimé sans
/// un clic de l'utilisateur sur « Purger maintenant ». Un plafond de taille
/// existait, appliqué en tâche de fond ; un seul gros fichier suffisait à
/// saturer le cache et à emporter tout le reste, alors il a été retiré.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Âge au-delà duquel une pièce jointe est purgée. `None` = on ne garde
    /// que le ménage des déchets (inachevés et orphelins).
    pub max_age: Option<Duration>,
}

/// Ce qu'une purge a libéré (ou libérerait, en simulation).
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

/// Purge le cache disque `media/`, **à la demande de l'utilisateur seulement**.
/// Deux catégories partent :
/// 1. les déchets — transferts inachevés (`.part`) et orphelins dont plus aucun
///    message ne parle — passé [`ORPHAN_GRACE`] ;
/// 2. les pièces jointes plus vieilles que `policy.max_age`, si une durée est
///    demandée.
///
/// Le message reste dans le fil dans tous les cas : seule la copie locale
/// disparaît, et la carte média l'annonce comme indisponible.
pub fn gc_media_dir(
    dir: PathBuf,
    referenced: HashSet<String>,
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

    // Garde-fou. Un dossier plein face à zéro référence est bien plus
    // probablement une anomalie — requête partielle, base rouverte ailleurs,
    // historique pas encore chargé — qu'un orphelinage général. Le 12 août
    // 2026, un GC a cru exactement cela et a supprimé 142 Mo de pièces jointes
    // pourtant référencées. Dans le doute, on ne juge plus personne orphelin.
    let references_trustworthy = !referenced.is_empty();

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
        let age = now
            .duration_since(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH))
            .unwrap_or_default();

        // Transfert inachevé : reconnu à son seul nom, sans rien demander à la
        // base. Supprimé passé le délai de grâce, qui protège une réception en
        // cours.
        if super::usage::is_part_file(name) {
            if age >= ORPHAN_GRACE {
                remove(&path, meta.len(), dry_run, &mut report);
            }
            continue;
        }

        // Orphelin : plus aucun message n'en parle. Le délai de grâce couvre
        // le bref instant où un média est écrit avant que son message
        // n'atteigne la base.
        if references_trustworthy && !referenced.contains(name) {
            if age >= ORPHAN_GRACE {
                remove(&path, meta.len(), dry_run, &mut report);
            }
            continue;
        }

        if policy.max_age.is_some_and(|max| age >= max) {
            remove(&path, meta.len(), dry_run, &mut report);
        }
    }

    if !references_trustworthy && report.freed_files == 0 && !dry_run {
        tracing::warn!("purge : aucune référence en base, aucun orphelin supprimé par précaution");
    }

    if report.freed_files > 0 && !dry_run {
        tracing::info!(
            "purge du cache : {} fichier(s) supprimé(s), {} octets libérés",
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
