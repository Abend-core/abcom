use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use super::group::GroupEvent;
use super::media::MediaAttachment;

/// Un message de chat sérialisé envoyé par TCP
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub from: String,
    pub content: String,
    /// Heure d'affichage `"%H:%M"` (repli pour les anciens messages / pairs).
    pub timestamp: String,
    /// Instant Unix (secondes), source de vérité pour la date et l'heure.
    /// Optionnel pour rester compatible avec les anciens messages et pairs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_epoch: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_user: Option<String>,
    /// Média (image ou fichier) joint au message, le cas échéant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaAttachment>,
    /// Hash du message auquel celui-ci répond (façon Discord), `None` sinon.
    /// Volontairement exclu du calcul de `AppState::message_hash`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<u64>,
    /// Nonce d'unicité tiré à la création : deux messages identiques envoyés
    /// dans la même seconde obtiennent quand même des hashs distincts
    /// (réactions, réponses et accusés ne se mélangent plus). `None` pour les
    /// anciens messages et pour les médias streamés (le hash du destinataire
    /// est reconstruit depuis `MediaStreamHeader`, qui ne porte pas de nonce ;
    /// leur `media.id` suffit à les distinguer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<u64>,
}

impl ChatMessage {
    /// Nonce frais pour un nouveau message : nanosecondes de l'horloge
    /// mélangées à un compteur de processus (pas de dépendance `rand`).
    /// Hash FNV-1a stable entre processus et machines.
    ///
    /// Contrairement à `DefaultHasher` (graine aléatoire depuis Rust 1.36),
    /// FNV-1a donne le même résultat partout, ce qui est indispensable : Alice
    /// le calcule à l'envoi, Bob le recalcule à la réception pour l'ACK — ils
    /// doivent tomber sur la même valeur.
    ///
    /// La clé inclut `timestamp_epoch`, `to_user` et `media.id` pour éviter les
    /// collisions entre messages identiques ou vides (cas des médias), plus le
    /// `nonce` quand il existe : deux messages au contenu identique envoyés
    /// dans la même seconde restent distincts. Le nonce n'est ajouté que s'il
    /// est présent, pour ne pas changer le hash des messages déjà persistés.
    ///
    /// `to_user` porte l'**identifiant** d'un salon, pas son nom : renommer un
    /// groupe laisse donc tous les hashs — et les réactions, accusés et repères
    /// de lecture qui s'y accrochent — intacts.
    pub fn stable_hash(&self) -> u64 {
        let key = format!(
            "{}:{}:{}:{}:{}{}",
            self.from,
            self.to_user.as_deref().unwrap_or("broadcast"),
            self.timestamp_epoch.unwrap_or(0),
            self.content,
            self.media.as_ref().map(|m| m.id.as_str()).unwrap_or(""),
            self.nonce.map(|n| format!(":{n}")).unwrap_or_default()
        );
        fnv1a(key.as_bytes())
    }

    pub fn fresh_nonce() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        nanos ^ COUNTER.fetch_add(1, Ordering::Relaxed).rotate_left(32)
    }
}

/// FNV-1a 64 bits : identique sur toutes les machines et tous les processus.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 14_695_981_039_346_656_037;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

/// Demande d'envoi d'un message à une adresse TCP
#[derive(Clone, Debug)]
pub struct SendRequest {
    pub to_peer: String,
    pub to_addr: SocketAddr,
    pub message: ChatMessage,
}

/// Demande d'envoi d'un événement de groupe à une adresse TCP
#[derive(Clone, Debug)]
pub struct SendGroupRequest {
    pub to_peer: String,
    pub to_addr: SocketAddr,
    pub event: GroupEvent,
}

#[cfg(test)]
#[path = "../tests/test_message_chat.rs"]
mod tests;
