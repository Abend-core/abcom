//! Configuration runtime dérivée de l'environnement.
//!
//! Permet de lancer plusieurs instances de l'application sur la même machine
//! (tests P2P locaux) via la variable d'environnement `ABCOM_INSTANCE`.
//! Chaque instance utilise des ports TCP distincts et un répertoire de données
//! séparé, tout en partageant le port UDP de découverte (broadcast LAN) afin de
//! se voir mutuellement.
//!
//! Instance 0 (ou variable absente) = comportement de production inchangé :
//! chat TCP 9000, transfert TCP 9001, données dans `abcom`.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Port UDP de découverte — partagé par toutes les instances (broadcast LAN).
pub const DISCOVERY_PORT: u16 = 9001;

/// Identifiant d'instance, lu une seule fois depuis `ABCOM_INSTANCE` (défaut 0).
pub fn instance_id() -> u32 {
    static ID: OnceLock<u32> = OnceLock::new();
    *ID.get_or_init(|| {
        std::env::var("ABCOM_INSTANCE")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0)
    })
}

/// Nombre d'instances distinctes adressables sans sortir de la plage de ports.
///
/// Chaque instance consomme `chat_port` et `chat_port + 1` ; au-delà, le calcul
/// déborderait `u16` — en release il rebouclait silencieusement sur des ports
/// déjà attribués, en debug il paniquait.
const MAX_INSTANCE_ID: u32 = (u16::MAX as u32 - 9001) / 10;

/// Port TCP de chat de cette instance : 9000, 9010, 9020, …
pub fn chat_port() -> u16 {
    let id = instance_id().min(MAX_INSTANCE_ID);
    9000 + (id as u16) * 10
}

/// Port TCP de streaming des médias : toujours `chat_port + 1`.
pub fn media_port() -> u16 {
    chat_port() + 1
}

/// Clé API Klipy, lue une seule fois depuis `ABCOM_KLIPY_API_KEY`.
///
/// Renvoie `None` si la variable est absente ou vide : dans ce cas, le bouton
/// GIF affiche une notification au lieu d'ouvrir le sélecteur.
pub fn klipy_api_key() -> Option<String> {
    static KEY: OnceLock<Option<String>> = OnceLock::new();
    KEY.get_or_init(|| {
        std::env::var("ABCOM_KLIPY_API_KEY")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    })
    .clone()
}

/// Répertoire de données : `abcom` pour l'instance 0, `abcom-N` sinon.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    match instance_id() {
        0 => base.join("abcom"),
        n => base.join(format!("abcom-{n}")),
    }
}

/// Durée de vie des fichiers de travail avant purge automatique.
pub const SCRATCH_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Fichiers de travail (collages longs en `.txt`), en 0700 — pas `/tmp`, lisible par les autres comptes.
pub fn scratch_dir() -> std::io::Result<PathBuf> {
    let dir = data_dir().join("scratch");
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Purge par ancienneté : le transfert média lit le fichier bien après la mise en file.
pub fn purge_scratch() {
    let Ok(dir) = scratch_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|modified| modified.elapsed().is_ok_and(|age| age > SCRATCH_TTL))
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
