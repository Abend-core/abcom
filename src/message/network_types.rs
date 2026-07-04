use serde::{Deserialize, Serialize};

/// Alias donné à un pair (nom convivial qui remplace le username à l'affichage)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerRecord {
    pub username: String,
    pub alias: Option<String>,
}

/// Paquet UDP pour la découverte des pairs sur le LAN
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiscoveryPacket {
    pub username: String,
    /// Port TCP de chat annoncé par l'émetteur. Absent des anciens paquets,
    /// auquel cas il vaut 9000 par défaut (rétro-compatibilité).
    #[serde(default = "default_chat_port")]
    pub port: u16,
    /// Clé publique X25519 (hexadécimal) de l'émetteur : lie l'annonce à
    /// l'identité vérifiée ensuite pendant le handshake Noise (TOFU).
    #[serde(default)]
    pub pubkey: String,
}

/// Identification échangée juste après le handshake Noise : chaque côté
/// annonce son username, que le récepteur lie à la clé statique reçue.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hello {
    pub username: String,
}

fn default_chat_port() -> u16 {
    9000
}

#[cfg(test)]
#[path = "../tests/test_message_network_types.rs"]
mod tests;
