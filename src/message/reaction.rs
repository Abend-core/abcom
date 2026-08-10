use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Action demandée sur une réaction (ajout ou retrait), transmise telle quelle
/// au pair distant pour qu'il applique le même changement (pas de toggle réseau
/// implicite : le côté local décide déjà add vs remove avant l'envoi).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReactionAction {
    Add,
    Remove,
}

/// Événement de réaction, envoyé/reçu tel quel entre pairs.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReactionEvent {
    pub message_hash: u64,
    pub emoji: String,
    pub user: String,
    pub action: ReactionAction,
}

/// Demande d'envoi d'un événement de réaction à une adresse TCP.
#[derive(Clone, Debug)]
pub struct ReactionRequest {
    pub to_peer: String,
    pub to_addr: SocketAddr,
    pub event: ReactionEvent,
}

/// Une entrée de réaction sur un message : un emoji et la liste des
/// utilisateurs ayant réagi avec celui-ci. Persistée dans SQLite,
/// indexée par `AppState::message_hash` du message ciblé.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ReactionEntry {
    pub emoji: String,
    pub users: Vec<String>,
}

#[cfg(test)]
#[path = "../tests/test_message_reaction.rs"]
mod tests;
