pub mod media_stream;
pub mod pool;
pub mod secure;
pub mod sender;
pub mod server;

pub use media_stream::*;
pub use pool::*;
pub use sender::*;
pub use server::*;

use std::sync::Arc;

use tokio::sync::mpsc::Sender;

use crate::identity::Identity;
use crate::message::AppEvent;
use secure::TrustStore;

/// Contexte partagé par toutes les tâches réseau : identité locale,
/// username, magasin TOFU et canal d'événements vers l'UI.
pub struct NetContext {
    pub identity: Identity,
    pub username: String,
    pub trust: Arc<TrustStore>,
    pub event_tx: Sender<AppEvent>,
    /// Passphrase de salon dérivée (32 octets) : si présente, le handshake
    /// utilise XXpsk3 — un pair sans la passphrase ne peut pas se connecter.
    pub psk: Option<Vec<u8>>,
}

impl NetContext {
    pub fn psk_bytes(&self) -> Option<&[u8]> {
        self.psk.as_deref()
    }
}

impl NetContext {
    /// Signale à l'UI qu'une clé de pair a changé (connexion refusée).
    pub async fn report_key_mismatch(&self, username: &str) {
        tracing::warn!(
            "clé inattendue pour « {username} » : connexion refusée (usurpation possible ?)"
        );
        let _ = self
            .event_tx
            .send(AppEvent::KeyChanged {
                username: username.to_string(),
            })
            .await;
    }
}
