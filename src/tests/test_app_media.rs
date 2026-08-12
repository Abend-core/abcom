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
        HashSet::new(),
        RetentionPolicy {
            max_age: Some(30 * DAY),
            ..RetentionPolicy::default()
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
        HashSet::new(),
        RetentionPolicy::default(),
        false,
    );

    assert!(dir.exists("tres-vieux.png"));
    assert_eq!(report.freed_files, 0);
}

/// Sous le plafond, une copie d'un fichier que nous avons envoyé part avant
/// une réception : nous en gardons l'original ailleurs sur la machine, alors
/// que la réception est notre seul exemplaire.
#[test]
fn ceiling_evicts_sent_copies_before_received() {
    let dir = MediaDir::new("ceiling");
    // L'envoi est le plus récent des deux : sans la priorité aux envois,
    // l'ancien tri par mtime aurait sacrifié la réception.
    dir.write_aged("recu.png", 600, 30 * DAY);
    dir.write_aged("envoye.png", 600, 10 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["recu.png", "envoye.png"]),
        ids(&["envoye.png"]),
        RetentionPolicy {
            max_age: None,
            max_bytes: Some(700),
        },
        false,
    );

    assert!(!dir.exists("envoye.png"), "l'envoi doit partir en premier");
    assert!(dir.exists("recu.png"), "la réception doit être préservée");
    assert_eq!(report.freed_bytes, 600);
    assert!(!report.over_ceiling);
}

/// Le scénario qui rendait l'application inutilisable : un seul gros fichier
/// dépasse à lui seul le plafond. L'ancien GC vidait alors tout le cache, y
/// compris des réceptions de la veille dont l'utilisateur n'a aucune autre
/// copie. Le plafond doit céder, pas les données.
#[test]
fn a_recent_oversized_file_never_wipes_the_cache() {
    let dir = MediaDir::new("oversized");
    dir.write_aged("enorme.zip", 5000, Duration::from_secs(60));
    dir.write_aged("hier.png", 400, 1 * DAY);
    dir.write_aged("avant-hier.png", 400, 2 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["enorme.zip", "hier.png", "avant-hier.png"]),
        HashSet::new(),
        RetentionPolicy {
            max_age: None,
            max_bytes: Some(1000),
        },
        false,
    );

    assert!(dir.exists("enorme.zip"));
    assert!(dir.exists("hier.png"));
    assert!(dir.exists("avant-hier.png"));
    assert_eq!(
        report.freed_files, 0,
        "aucune donnée récente ne doit partir"
    );
    assert!(
        report.over_ceiling,
        "le dépassement doit être signalé à l'utilisateur"
    );
}

/// Le plancher de protection ne vaut que pour le plafond : une durée de
/// conservation est un choix explicite, elle s'applique même en deçà.
#[test]
fn the_age_rule_overrides_the_ceiling_protection_floor() {
    let dir = MediaDir::new("floor");
    dir.write_aged("recent.png", 100, 2 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["recent.png"]),
        HashSet::new(),
        RetentionPolicy {
            max_age: Some(1 * DAY),
            max_bytes: None,
        },
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
        HashSet::new(),
        RetentionPolicy::default(),
        false,
    );

    assert!(dir.exists("qui-vient-d-arriver.png"));
    assert_eq!(report.freed_files, 0);
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
        HashSet::new(),
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
