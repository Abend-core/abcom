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

/// Sans règle d'âge, rien ne bouge tant que le plafond n'est pas dépassé.
#[test]
fn unlimited_retention_keeps_everything_under_the_ceiling() {
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
    // L'envoi est le plus récent : sans la priorité aux envois, l'ancien tri
    // par mtime aurait sacrifié la réception.
    dir.write_aged("recu.png", 600, 10 * DAY);
    dir.write_aged("envoye.png", 600, 1 * DAY);

    let report = gc_media_dir(
        dir.path().to_path_buf(),
        ids(&["recu.png", "envoye.png"]),
        ids(&["envoye.png"]),
        RetentionPolicy {
            max_age: None,
            max_bytes: 700,
        },
        false,
    );

    assert!(!dir.exists("envoye.png"), "l'envoi doit partir en premier");
    assert!(dir.exists("recu.png"), "la réception doit être préservée");
    assert_eq!(report.freed_bytes, 600);
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
