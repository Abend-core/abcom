use crate::app::media::{gc_media_dir, RetentionPolicy};
use crate::app::AppState;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Répertoire de médias temporaire, effacé même si le test panique.
struct MediaDir(PathBuf);

impl MediaDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "abcom-gc-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Écrit un fichier et le vieillit : le GC ne juge que sur le mtime.
    fn write_aged(&self, name: &str, bytes: usize, age: Duration) {
        let path = self.0.join(name);
        std::fs::write(&path, vec![b'x'; bytes]).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
    }

    fn exists(&self, name: &str) -> bool {
        self.0.join(name).exists()
    }
}

impl Drop for MediaDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ids(names: &[&str]) -> HashSet<String> {
    names.iter().map(|n| (*n).to_string()).collect()
}

const DAY: Duration = Duration::from_secs(86_400);

/// Le GC périodique effaçait le fichier temporaire d'une réception **en
/// cours** : il n'est référencé par aucun message, donc il passait pour un
/// orphelin. Un transfert récent doit survivre.
#[test]
fn gc_spares_a_transfer_in_progress() {
    let dir = MediaDir::new("inflight");
    dir.write_aged(".abcom-42-1.part", 500, Duration::from_secs(30));

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        HashSet::new(),
        RetentionPolicy::default(),
        false,
    );

    assert!(
        dir.exists(".abcom-42-1.part"),
        "un transfert en cours ne doit pas être ramassé"
    );
    assert_eq!(report.freed_files, 0);
}

/// Passé le délai de grâce, le même fichier est un déchet : plus personne ne
/// reprendra ce transfert.
#[test]
fn gc_removes_an_abandoned_part_file() {
    let dir = MediaDir::new("abandoned");
    dir.write_aged(".abcom-42-1.part", 500, Duration::from_secs(6 * 3600));

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        HashSet::new(),
        RetentionPolicy::default(),
        false,
    );

    assert!(!dir.exists(".abcom-42-1.part"));
    assert_eq!(report.freed_bytes, 500);
}

/// La règle d'âge élague les pièces jointes trop vieilles, et seulement elles.
#[test]
fn retention_purges_by_age() {
    let dir = MediaDir::new("age");
    dir.write_aged("vieux.png", 100, 40 * DAY);
    dir.write_aged("recent.png", 200, 2 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["vieux.png", "recent.png"]),
        RetentionPolicy {
            max_age: Some(30 * DAY),
        },
        false,
    );

    assert!(!dir.exists("vieux.png"));
    assert!(dir.exists("recent.png"));
    assert_eq!(report.freed_bytes, 100);
}

/// Réglages par défaut : rien ne disparaît, quel que soit l'âge. Une mise à
/// jour de l'application ne doit coûter aucune pièce jointe.
#[test]
fn default_policy_deletes_nothing() {
    let dir = MediaDir::new("unlimited");
    dir.write_aged("tres-vieux.png", 100, 3000 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["tres-vieux.png"]),
        RetentionPolicy::default(),
        false,
    );

    assert!(dir.exists("tres-vieux.png"));
    assert_eq!(report.freed_files, 0);
}

/// Le scénario qui rendait l'application inutilisable : un gros fichier
/// saturait le cache et le plafond emportait tout le reste, y compris des
/// réceptions de la veille dont l'utilisateur n'a aucune autre copie. Il n'y a
/// plus ni plafond ni purge de fond : la taille n'est plus un motif de
/// suppression, seul l'âge demandé par l'utilisateur en est un.
#[test]
fn size_alone_never_deletes_anything() {
    let dir = MediaDir::new("oversized");
    dir.write_aged("enorme.zip", 5_000_000, Duration::from_secs(60));
    dir.write_aged("hier.png", 400, DAY);
    dir.write_aged("avant-hier.png", 400, 2 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["enorme.zip", "hier.png", "avant-hier.png"]),
        RetentionPolicy::default(),
        false,
    );

    assert!(dir.exists("enorme.zip"));
    assert!(dir.exists("hier.png"));
    assert!(dir.exists("avant-hier.png"));
    assert_eq!(report.freed_files, 0);
}

/// Une durée demandée s'applique telle quelle, sans plancher de protection :
/// c'est un choix explicite de l'utilisateur, pas une réaction automatique.
#[test]
fn the_requested_age_applies_verbatim() {
    let dir = MediaDir::new("floor");
    dir.write_aged("recent.png", 100, 2 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["recent.png"]),
        RetentionPolicy { max_age: Some(DAY) },
        false,
    );

    assert!(!dir.exists("recent.png"));
    assert_eq!(report.freed_bytes, 100);
}

/// Un média fraîchement reçu existe sur le disque un court instant avant que
/// son message n'atteigne la base : le prendre pour un orphelin et l'effacer
/// perdait le fichier à l'arrivée.
#[test]
fn a_just_written_media_is_not_mistaken_for_an_orphan() {
    let dir = MediaDir::new("fresh");
    dir.write_aged("qui-vient-d-arriver.png", 100, Duration::from_secs(5));

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        HashSet::new(),
        RetentionPolicy::default(),
        false,
    );

    assert!(dir.exists("qui-vient-d-arriver.png"));
    assert_eq!(report.freed_files, 0);
}

/// Rien ne doit partir sans clic : c'est toute la promesse de la purge
/// manuelle. Une pièce jointe référencée, quel que soit son âge, survit tant
/// qu'aucune durée n'est demandée.
#[test]
fn nothing_is_purged_without_a_requested_age() {
    let dir = MediaDir::new("manual");
    dir.write_aged("vieille.png", 100, 500 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["vieille.png"]),
        RetentionPolicy { max_age: None },
        false,
    );

    assert!(dir.exists("vieille.png"));
    assert_eq!(report.freed_files, 0);
}

/// Le 12 août 2026, un GC a reçu un ensemble de références vide devant un
/// dossier plein, a conclu que tout était orphelin, et a supprimé 142 Mo de
/// pièces jointes pourtant référencées en base. Zéro référence n'est pas une
/// autorisation de tout effacer.
#[test]
fn an_empty_reference_set_deletes_no_attachment() {
    let dir = MediaDir::new("noref");
    dir.write_aged("precieux.png", 100, 10 * DAY);
    dir.write_aged("aussi-precieux.zip", 200, 10 * DAY);
    // Le déchet reconnaissable à son nom part quand même : il ne dépend
    // d'aucune requête.
    dir.write_aged(".abcom-9-1.part", 50, 10 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        HashSet::new(),
        RetentionPolicy::default(),
        false,
    );

    assert!(dir.exists("precieux.png"));
    assert!(dir.exists("aussi-precieux.zip"));
    assert!(!dir.exists(".abcom-9-1.part"));
    assert_eq!(report.freed_bytes, 50);
}

/// L'aperçu de Paramètres annonce ce qu'une purge libérerait : il doit
/// compter juste sans rien supprimer.
#[test]
fn dry_run_reports_without_deleting() {
    let dir = MediaDir::new("dryrun");
    dir.write_aged("orphelin.png", 300, 1 * DAY);
    dir.write_aged("garde.png", 50, 1 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["garde.png"]),
        RetentionPolicy::default(),
        true,
    );

    assert!(report.dry_run);
    assert_eq!(report.freed_bytes, 300);
    assert_eq!(report.freed_files, 1);
    assert!(
        dir.exists("orphelin.png"),
        "une simulation ne supprime rien"
    );
}

#[test]
fn media_path_under_data_dir() {
    let dir = std::env::temp_dir().join(format!("abcom_media_{}", std::process::id()));
    let s = AppState::new_with_base("alice", &dir);
    assert_eq!(s.media_path("x.bin"), dir.join("media").join("x.bin"));
    assert!(s.media_bytes("absent.bin").is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn media_bytes_reads_written_file() {
    let dir = std::env::temp_dir().join(format!("abcom_media2_{}", std::process::id()));
    let s = AppState::new_with_base("alice", &dir);
    std::fs::create_dir_all(dir.join("media")).unwrap();
    std::fs::write(s.media_path("y.bin"), [7, 8, 9]).unwrap();
    assert_eq!(s.media_bytes("y.bin"), Some(vec![7, 8, 9]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn remove_media_message_drops_message_and_file() {
    use crate::message::{ChatMessage, MediaAttachment, MediaKind};
    let dir = std::env::temp_dir().join(format!("abcom_media3_{}", std::process::id()));
    let mut s = AppState::new_with_base("alice", &dir);
    std::fs::create_dir_all(dir.join("media")).unwrap();
    std::fs::write(s.media_path("z.bin"), [1, 2, 3]).unwrap();
    s.messages.push(ChatMessage {
        from: "bob".to_string(),
        content: String::new(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: None,
        to_user: Some("alice".to_string()),
        media: Some(MediaAttachment {
            id: "z.bin".to_string(),
            filename: "z.bin".to_string(),
            kind: MediaKind::File,
            size_bytes: 3,
            url: None,
            width: None,
            height: None,
        }),
        reply_to: None,
        nonce: None,
    });

    s.remove_media_message("z.bin");

    assert!(s.messages.is_empty(), "le message média doit être retiré");
    assert!(
        s.media_bytes("z.bin").is_none(),
        "le fichier doit être supprimé"
    );
    std::fs::remove_dir_all(&dir).ok();
}
