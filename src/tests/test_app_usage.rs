use super::{scan, Usage};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Répertoire temporaire supprimé à la fin du test, même en cas de panique.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "abcom-usage-{tag}-{}-{:?}",
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
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(path: PathBuf, bytes: usize) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, vec![b'x'; bytes]).unwrap();
}

fn ids(names: &[&str]) -> HashSet<String> {
    names.iter().map(|n| (*n).to_string()).collect()
}

/// Un répertoire vierge ne doit rien inventer : c'est l'état au tout premier
/// lancement, avant le moindre média.
#[test]
fn an_empty_data_dir_reports_nothing() {
    let dir = TempDir::new("empty");
    let usage = scan(dir.path(), &dir.path().join("media"), &HashSet::new());

    assert_eq!(usage, Usage::default());
    assert_eq!(usage.total().bytes, 0);
    assert_eq!(usage.total().files, 0);
}

/// Le dossier `media/` mélange les deux sens sous des noms opaques : seule la
/// liste d'identifiants tirée de l'historique permet de les séparer.
#[test]
fn media_are_split_between_sent_and_received() {
    let dir = TempDir::new("media");
    let media = dir.path().join("media");
    write(media.join("aaa.png"), 100);
    write(media.join("bbb.png"), 250);
    write(media.join("ccc.zip"), 400);

    let usage = scan(dir.path(), &media, &ids(&["bbb.png", "ccc.zip"]));

    assert_eq!(usage.media_received.bytes, 100);
    assert_eq!(usage.media_received.files, 1);
    assert_eq!(usage.media_sent.bytes, 650);
    assert_eq!(usage.media_sent.files, 2);
}

/// Un fichier absent de la liste des envois est compté comme reçu : on ne doit
/// jamais présenter comme « copie superflue » un fichier dont l'utilisateur
/// n'a pas l'original ailleurs.
#[test]
fn an_unknown_media_counts_as_received() {
    let dir = TempDir::new("unknown");
    let media = dir.path().join("media");
    write(media.join("orphelin.bin"), 90);

    let usage = scan(dir.path(), &media, &ids(&["autre.bin"]));

    assert_eq!(usage.media_received.bytes, 90);
    assert_eq!(usage.media_sent, Default::default());
}

/// Les fichiers annexes de SQLite pèsent parfois autant que la base : les
/// omettre sous-estimerait le poste au moment où il compte le plus.
#[test]
fn database_includes_its_wal_and_shm_companions() {
    let dir = TempDir::new("db");
    write(dir.path().join("abcom.db"), 1_000);
    write(dir.path().join("abcom.db-wal"), 500);
    write(dir.path().join("abcom.db-shm"), 32);

    let usage = scan(dir.path(), &dir.path().join("media"), &HashSet::new());

    assert_eq!(usage.database.bytes, 1_532);
    assert_eq!(usage.database.files, 3);
}

#[test]
fn logs_scratch_and_avatar_are_each_counted_once() {
    let dir = TempDir::new("misc");
    write(dir.path().join("logs/abcom.log.2026-08-11"), 300);
    write(dir.path().join("logs/abcom.log.2026-08-12"), 200);
    write(dir.path().join("scratch/collage.txt"), 50);
    write(dir.path().join("avatar.png"), 64);

    let usage = scan(dir.path(), &dir.path().join("media"), &HashSet::new());

    assert_eq!(usage.logs.bytes, 500);
    assert_eq!(usage.logs.files, 2);
    assert_eq!(usage.scratch.bytes, 50);
    assert_eq!(usage.avatar.bytes, 64);
    assert_eq!(usage.avatar.files, 1);
}

/// Le total doit couvrir tous les postes : en oublier un afficherait une somme
/// inférieure à la place réellement occupée.
#[test]
fn total_sums_every_bucket() {
    let dir = TempDir::new("total");
    let media = dir.path().join("media");
    write(dir.path().join("abcom.db"), 1);
    write(media.join("recu.png"), 2);
    write(media.join("envoye.png"), 4);
    write(dir.path().join("avatar.png"), 8);
    write(dir.path().join("logs/a.log"), 16);
    write(dir.path().join("scratch/b.txt"), 32);

    let usage = scan(dir.path(), &media, &ids(&["envoye.png"]));

    assert_eq!(usage.total().bytes, 63);
    assert_eq!(usage.total().files, 6);
}

/// Un transfert interrompu laisse un `.part` que rien ne référence : le
/// compter en « reçus » faisait passer un déchet pour une pièce jointe.
#[test]
fn part_files_are_counted_as_incomplete() {
    let dir = TempDir::new("part");
    let media = dir.path().join("media");
    write(media.join("recu.png"), 100);
    write(media.join(".abcom-42-1.part"), 700);

    let usage = scan(dir.path(), &media, &HashSet::new());

    assert_eq!(usage.incomplete.bytes, 700);
    assert_eq!(usage.incomplete.files, 1);
    assert_eq!(usage.media_received.bytes, 100);
}

/// Le total affiché doit être le poids réel du dossier. Tant que la clé
/// d'identité, `networks.json` et consorts n'étaient comptés nulle part,
/// « Espace occupé » annonçait moins que ce que l'utilisateur voyait dans son
/// explorateur de fichiers.
#[test]
fn unaccounted_files_land_in_other() {
    let dir = TempDir::new("other");
    let media = dir.path().join("media");
    write(dir.path().join("abcom.db"), 10);
    write(media.join("recu.png"), 20);
    write(dir.path().join("identity.key"), 64);
    write(dir.path().join("networks.json"), 118);
    write(dir.path().join("last-panic.txt"), 8);
    // Sous-dossier inconnu : compté récursivement, sinon le total ment encore.
    write(dir.path().join("inconnu/quelque-chose.bin"), 40);

    let usage = scan(dir.path(), &media, &HashSet::new());

    assert_eq!(usage.other.bytes, 64 + 118 + 8 + 40);
    assert_eq!(usage.other.files, 4);
    assert_eq!(usage.total().bytes, 260);
    assert_eq!(usage.total().files, 6);
}
