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

#[cfg(test)]
mod tests {
    use super::{KnownNetwork, PeerRecord, DiscoveryPacket};

    fn net(id: &str, subnet: &str, alias: Option<&str>) -> KnownNetwork {
        KnownNetwork {
            id: id.to_string(),
            subnet: subnet.to_string(),
            alias: alias.map(|s| s.to_string()),
            seen_peers: vec![],
        }
    }

    // ── KnownNetwork::display_name ──────────────────────────────────────────

    #[test]
    fn display_name_prefers_alias() {
        let n = net("ssid-abc", "192.168.1", Some("Maison"));
        assert_eq!(n.display_name(), "Maison");
    }

    #[test]
    fn display_name_falls_back_to_id() {
        let n = net("ssid-abc", "192.168.1", None);
        assert_eq!(n.display_name(), "ssid-abc");
    }

    #[test]
    fn display_name_falls_back_to_subnet_dot_x() {
        let n = net("", "192.168.1", None);
        assert_eq!(n.display_name(), "192.168.1.x");
    }

    #[test]
    fn display_name_all_empty_returns_dot_x() {
        let n = net("", "", None);
        assert_eq!(n.display_name(), ".x");
    }

    // ── Sérialisation JSON ──────────────────────────────────────────────────

    #[test]
    fn known_network_round_trip() {
        let mut n = net("ssid-home", "192.168.0", Some("Bureau"));
        n.seen_peers = vec!["alice".to_string(), "bob".to_string()];
        let json = serde_json::to_string(&n).unwrap();
        let decoded: KnownNetwork = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "ssid-home");
        assert_eq!(decoded.alias, Some("Bureau".to_string()));
        assert_eq!(decoded.seen_peers.len(), 2);
    }

    #[test]
    fn peer_record_round_trip_with_alias() {
        let r = PeerRecord {
            username: "bob".to_string(),
            alias: Some("Robert".to_string()),
            last_subnet: Some("192.168.1".to_string()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let decoded: PeerRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.username, "bob");
        assert_eq!(decoded.alias, Some("Robert".to_string()));
        assert_eq!(decoded.last_subnet, Some("192.168.1".to_string()));
    }

    #[test]
    fn peer_record_round_trip_no_alias() {
        let r = PeerRecord { username: "charlie".to_string(), alias: None, last_subnet: None };
        let json = serde_json::to_string(&r).unwrap();
        let decoded: PeerRecord = serde_json::from_str(&json).unwrap();
        assert!(decoded.alias.is_none());
        assert!(decoded.last_subnet.is_none());
    }

    #[test]
    fn discovery_packet_round_trip() {
        let p = DiscoveryPacket { username: "alice".to_string() };
        let json = serde_json::to_string(&p).unwrap();
        let decoded: DiscoveryPacket = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.username, "alice");
    }

    #[test]
    fn known_network_default_fields_from_partial_json() {
        // id et subnet ont #[serde(default)] → absents du JSON
        let json = r#"{"alias":null,"seen_peers":["alice"]}"#;
        let n: KnownNetwork = serde_json::from_str(json).unwrap();
        assert_eq!(n.id, "");
        assert_eq!(n.subnet, "");
        assert_eq!(n.seen_peers.len(), 1);
    }
}
