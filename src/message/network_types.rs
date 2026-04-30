use serde::{Deserialize, Serialize};

/// Réseau connu (identifié par SSID WiFi si disponible, sinon subnet)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KnownNetwork {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub subnet: String,
    pub alias: Option<String>,
    pub seen_peers: Vec<String>,
}

impl KnownNetwork {
    pub fn display_name(&self) -> String {
        if let Some(alias) = &self.alias {
            alias.clone()
        } else if !self.id.is_empty() {
            self.id.clone()
        } else {
            format!("{}.x", self.subnet)
        }
    }
}

/// Alias donné à un pair
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PeerRecord {
    pub username: String,
    pub alias: Option<String>,
    pub last_subnet: Option<String>,
}

/// Paquet UDP pour la découverte des pairs sur le LAN
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiscoveryPacket {
    pub username: String,
}
