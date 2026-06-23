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

/// Port TCP de chat de cette instance : 9000, 9010, 9020, …
pub fn chat_port() -> u16 {
    9000 + (instance_id() as u16) * 10
}

/// Port TCP de transfert de fichiers : toujours `chat_port + 1`.
pub fn transfer_port() -> u16 {
    chat_port() + 1
}

/// Répertoire de données : `abcom` pour l'instance 0, `abcom-N` sinon.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    match instance_id() {
        0 => base.join("abcom"),
        n => base.join(format!("abcom-{n}")),
    }
}
