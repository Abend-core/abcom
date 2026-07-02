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
