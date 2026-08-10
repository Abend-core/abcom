use crate::message::DiscoveryPacket;

#[test]
fn discovery_packet_round_trip() {
    let pkt = DiscoveryPacket {
        username: "alice".to_string(),
        port: 9000,
        pubkey: "abcd".to_string(),
        ..Default::default()
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

#[test]
fn signed_announcement_is_accepted_and_tampering_is_not() {
    let identity = crate::identity::Identity::ephemeral().unwrap();
    let key = identity.signing_key();
    let template = DiscoveryPacket {
        username: "alice".to_string(),
        port: 9000,
        pubkey: identity.public_hex(),
        verifying_key: identity.verifying_hex(),
        ..Default::default()
    };

    let bytes = super::sign_announcement(&template, &key, 1_000);
    let signed: DiscoveryPacket = serde_json::from_slice(&bytes).unwrap();
    assert!(super::announcement_is_authentic(&signed, 1_000));

    // Champ modifié après signature : le pseudo d'un autre ne passe pas.
    let mut usurped = signed.clone();
    usurped.username = "bob".to_string();
    assert!(!super::announcement_is_authentic(&usurped, 1_000));

    // Adresse détournée : le port fait partie de la charge signée.
    let mut redirected = signed.clone();
    redirected.port = 9999;
    assert!(!super::announcement_is_authentic(&redirected, 1_000));

    // Rejeu d'une annonce capturée trop ancienne.
    assert!(!super::announcement_is_authentic(&signed, 1_000 + 3_600));

    // Annonce non signée (ancien pair ou fabrication).
    assert!(!super::announcement_is_authentic(&template, 1_000));
}

#[test]
fn a_foreign_key_cannot_sign_for_another_identity() {
    let alice = crate::identity::Identity::ephemeral().unwrap();
    let mallory = crate::identity::Identity::ephemeral().unwrap();
    // Mallory annonce la clé d'Alice mais ne peut pas la signer.
    let template = DiscoveryPacket {
        username: "alice".to_string(),
        port: 9000,
        pubkey: alice.public_hex(),
        verifying_key: alice.verifying_hex(),
        ..Default::default()
    };
    let bytes = super::sign_announcement(&template, &mallory.signing_key(), 1_000);
    let forged: DiscoveryPacket = serde_json::from_slice(&bytes).unwrap();
    assert!(!super::announcement_is_authentic(&forged, 1_000));
}

#[test]
fn a_backward_clock_jump_does_not_drop_every_peer() {
    // Pair vu à l'instant : pas encore périmé.
    assert!(!super::peer_is_stale(1_000, 1_000));
    // Silencieux au-delà du délai : périmé.
    assert!(super::peer_is_stale(
        1_000 + super::DISCOVERY_TIMEOUT,
        1_000
    ));
    // Horloge reculée de 100 s : `now - last_seen` déborderait et déclarerait
    // le pair perdu. Il doit rester considéré comme frais.
    assert!(!super::peer_is_stale(1_000, 1_100));
}
