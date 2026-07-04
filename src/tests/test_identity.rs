use super::*;

fn tmp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "abcom-identity-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn generates_then_reloads_same_keypair() {
    let dir = tmp_dir();
    let first = Identity::load_or_create(&dir).unwrap();
    assert_eq!(first.private.len(), 32);
    assert_eq!(first.public.len(), 32);
    assert!(dir.join("identity.key").exists());

    let second = Identity::load_or_create(&dir).unwrap();
    assert_eq!(first.public, second.public);
    assert_eq!(first.private, second.private);
}

#[test]
fn ephemeral_keys_are_distinct() {
    let a = Identity::ephemeral().unwrap();
    let b = Identity::ephemeral().unwrap();
    assert_ne!(a.public, b.public);
}

#[test]
fn fingerprint_is_stable_and_readable() {
    let id = Identity::ephemeral().unwrap();
    let fp = id.fingerprint();
    // 8 groupes de 4 hexa séparés par « : ».
    assert_eq!(fp.split(':').count(), 8);
    assert!(fp.split(':').all(|g| g.len() == 4));
    assert_eq!(fp, fingerprint(&id.public));
}

#[test]
fn invalid_key_file_is_regenerated() {
    let dir = tmp_dir();
    std::fs::write(dir.join("identity.key"), b"corrompu").unwrap();
    let id = Identity::load_or_create(&dir).unwrap();
    assert_eq!(id.public.len(), 32);
    // Le fichier a été réécrit avec une paire valide.
    assert_eq!(std::fs::read(dir.join("identity.key")).unwrap().len(), 64);
}
