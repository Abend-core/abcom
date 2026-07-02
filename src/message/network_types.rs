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
}

fn default_chat_port() -> u16 {
    9000
}

#[cfg(test)]
#[path = "../tests/test_message_network_types.rs"]
mod tests;
