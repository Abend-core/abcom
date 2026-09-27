use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Annonce réseau de l'image de profil (avatar) d'un utilisateur.
///
/// Les octets `png` sont une image PNG normalisée (carrée et compacte, voir
/// `ui::avatar`), suffisamment petite pour être transmise par TCP comme les
/// autres messages. Un vecteur vide signale le retrait de l'avatar.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AvatarAnnounce {
    pub from: String,
    pub png: Vec<u8>,
}

/// Demande d'envoi d'un avatar à une adresse TCP.
#[derive(Clone, Debug)]
pub struct AvatarRequest {
    pub to_peer: String,
    pub to_addr: SocketAddr,
    pub announce: AvatarAnnounce,
}

#[cfg(test)]
#[path = "../tests/test_message_avatar.rs"]
mod tests;
