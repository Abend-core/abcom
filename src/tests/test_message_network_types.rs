
use super::{DiscoveryPacket, PeerRecord};

#[test]
fn peer_record_round_trip_with_alias() {
    let r = PeerRecord {
        username: "bob".to_string(),
        alias: Some("Robert".to_string()),
    };
    let json = serde_json::to_string(&r).unwrap();
    let decoded: PeerRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.username, "bob");
    assert_eq!(decoded.alias, Some("Robert".to_string()));
}

#[test]
fn peer_record_round_trip_no_alias() {
    let r = PeerRecord {
        username: "charlie".to_string(),
        alias: None,
    };
    let json = serde_json::to_string(&r).unwrap();
    let decoded: PeerRecord = serde_json::from_str(&json).unwrap();
    assert!(decoded.alias.is_none());
}

#[test]
fn discovery_packet_round_trip() {
    let p = DiscoveryPacket {
        username: "alice".to_string(),
        port: 9010,
    };
    let json = serde_json::to_string(&p).unwrap();
    let decoded: DiscoveryPacket = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.username, "alice");
    assert_eq!(decoded.port, 9010);
}

#[test]
fn discovery_packet_legacy_without_port_defaults_to_9000() {
    // Ancien paquet sans champ `port` → 9000 par défaut
    let json = r#"{"username":"bob"}"#;
    let decoded: DiscoveryPacket = serde_json::from_str(json).unwrap();
    assert_eq!(decoded.username, "bob");
    assert_eq!(decoded.port, 9000);
}

#[test]
fn peer_record_ignores_legacy_last_subnet_field() {
    // Anciens enregistrements avec `last_subnet` → champ inconnu ignoré
    let json = r#"{"username":"bob","alias":"Robert","last_subnet":"192.168.1"}"#;
    let decoded: PeerRecord = serde_json::from_str(json).unwrap();
    assert_eq!(decoded.username, "bob");
    assert_eq!(decoded.alias, Some("Robert".to_string()));
}
