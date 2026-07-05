use crate::message::DiscoveryPacket;

#[test]
fn discovery_packet_round_trip() {
    let pkt = DiscoveryPacket {
        username: "alice".to_string(),
        port: 9000,
        pubkey: "abcd".to_string(),
    };
    let json = serde_json::to_string(&pkt).unwrap();
    let decoded: DiscoveryPacket = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.username, "alice");
    assert_eq!(decoded.port, 9000);
}

#[test]
fn discovery_packet_legacy_without_port() {
    // Anciens clients qui n'envoient pas le champ `port`
    let json = r#"{"username":"bob"}"#;
    let decoded: DiscoveryPacket = serde_json::from_str(json).unwrap();
    assert_eq!(decoded.username, "bob");
    assert_eq!(
        decoded.port, 9000,
        "port absent doit valoir 9000 (rétro-compat)"
    );
}

#[test]
fn discovery_packet_ignores_unknown_fields() {
    let json = r#"{"username":"charlie","port":8888,"extra":"ignored"}"#;
    let decoded: DiscoveryPacket = serde_json::from_str(json).unwrap();
    assert_eq!(decoded.username, "charlie");
    assert_eq!(decoded.port, 8888);
}

#[tokio::test]
async fn bind_discovery_socket_succeeds() {
    // Vérifie que la création du socket UDP (SO_REUSEADDR + broadcast) fonctionne.
    // Deux instances doivent pouvoir partager le même port — c'est l'invariant clé.
    let result = super::bind_discovery_socket();
    assert!(
        result.is_ok(),
        "bind_discovery_socket a échoué: {:?}",
        result.err()
    );
}
