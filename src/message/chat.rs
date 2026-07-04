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

/// Demande d'envoi d'un message à une adresse TCP
#[derive(Clone, Debug)]
pub struct SendRequest {
    pub to_addr: SocketAddr,
    pub message: ChatMessage,
}

/// Demande d'envoi d'un événement de groupe à une adresse TCP
#[derive(Clone, Debug)]
pub struct SendGroupRequest {
    pub to_addr: SocketAddr,
    pub event: GroupEvent,
}

#[cfg(test)]
#[path = "../tests/test_message_chat.rs"]
mod tests;
