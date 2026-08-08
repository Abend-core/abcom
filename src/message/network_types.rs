use serde::{Deserialize, Serialize};

/// Alias donné à un pair (nom convivial qui remplace le username à l'affichage)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerRecord {
    pub username: String,
    pub alias: Option<String>,
}

/// Paquet UDP pour la découverte des pairs sur le LAN
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DiscoveryPacket {
    pub username: String,
    /// Port TCP de chat annoncé par l'émetteur. Absent des anciens paquets,
    /// auquel cas il vaut 9000 par défaut (rétro-compatibilité).
    #[serde(default = "default_chat_port")]
    pub port: u16,
    /// Clé publique X25519 annoncée à titre informatif. La source de vérité
    /// reste la clé présentée et épinglée pendant le handshake Noise (TOFU).
    #[serde(default)]
    pub pubkey: String,
    /// Clé Ed25519 de vérification, dérivée de l'identité Noise de l'émetteur.
    #[serde(default)]
    pub verifying_key: String,
    /// Instant d'émission (epoch, secondes) : borne la fenêtre de rejeu.
    #[serde(default)]
    pub sent_at: u64,
    /// Signature Ed25519 du corps de l'annonce (cf. `signed_payload`).
    #[serde(default)]
    pub signature: String,
}

impl DiscoveryPacket {
    /// Octets couverts par la signature : tout ce qui identifie l'annonce.
    pub fn signed_payload(&self) -> Vec<u8> {
        format!(
            "abcom-discovery-v1|{}|{}|{}|{}|{}",
            self.username, self.port, self.pubkey, self.verifying_key, self.sent_at
        )
        .into_bytes()
    }
}

/// Identification échangée juste après le handshake Noise : chaque côté
/// annonce son username, que le récepteur lie à la clé statique reçue.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hello {
    pub username: String,
    pub protocol_version: u16,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

fn default_chat_port() -> u16 {
    9000
}

#[cfg(test)]
#[path = "../tests/test_message_network_types.rs"]
mod tests;
