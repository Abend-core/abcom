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
    pub to_addr: SocketAddr,
    pub announce: AvatarAnnounce,
}

#[cfg(test)]
mod tests {
    use super::AvatarAnnounce;

    #[test]
    fn avatar_announce_round_trip() {
        let a = AvatarAnnounce { from: "alice".to_string(), png: vec![1, 2, 3, 4] };
        let json = serde_json::to_string(&a).unwrap();
        let decoded: AvatarAnnounce = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.from, "alice");
        assert_eq!(decoded.png, vec![1, 2, 3, 4]);
    }

    #[test]
    fn avatar_announce_empty_marks_removal() {
        let a = AvatarAnnounce { from: "bob".to_string(), png: Vec::new() };
        let json = serde_json::to_string(&a).unwrap();
        let decoded: AvatarAnnounce = serde_json::from_str(&json).unwrap();
        assert!(decoded.png.is_empty());
    }
}
