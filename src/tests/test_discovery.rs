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

/// Régression : une même annonce part en multicast **et** en broadcast, et
/// nous revient donc sous deux adresses source. Tant que le pair répond,
/// l'application ne doit voir qu'une seule adresse, annoncée une seule fois.
///
/// Sans cela l'adresse du pair basculait à chaque annonce : le pool rouvrait
/// une connexion à chaque bascule, la session en double était refusée d'en
/// face, et les messages émis entre-temps disparaissaient sans erreur (mesuré
/// à 69 % de pertes entre deux instances locales).
#[test]
fn the_two_channels_of_one_announcement_yield_one_discovery() {
    use std::collections::HashMap;
    use std::net::SocketAddr;

    let loopback: SocketAddr = "127.0.0.1:9010".parse().unwrap();
    let lan: SocketAddr = "192.168.1.39:9010".parse().unwrap();
    let mut timestamps = HashMap::new();
    let mut addrs = HashMap::new();

    let mut announced = Vec::new();
    // Dix cycles d'annonce, chacun reçu par les deux canaux.
    for cycle in 0..10 {
        let now = 1_000 + cycle * super::BROADCAST_INTERVAL;
        for source in [loopback, lan] {
            let seen =
                super::observe_announcement(&mut timestamps, &mut addrs, "alice", source, now);
            announced.extend(seen);
        }
    }
    assert_eq!(
        announced,
        vec![loopback],
        "un pair stable ne doit être annoncé qu'une fois, à une seule adresse"
    );
}

#[test]
fn the_tracked_peer_cap_still_holds() {
    use std::collections::HashMap;
    use std::net::SocketAddr;

    let mut timestamps = HashMap::new();
    let mut addrs = HashMap::new();
    let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
    for i in 0..super::MAX_TRACKED_PEERS {
        assert!(super::observe_announcement(
            &mut timestamps,
            &mut addrs,
            &format!("p{i}"),
            addr,
            1_000
        )
        .is_some());
    }
    assert!(
        super::observe_announcement(&mut timestamps, &mut addrs, "un-de-trop", addr, 1_000)
            .is_none(),
        "au-delà du plafond, un pair inconnu est ignoré"
    );
    assert!(
        super::observe_announcement(&mut timestamps, &mut addrs, "p0", addr, 1_100).is_none(),
        "un pair déjà suivi reste rafraîchi sans nouvel événement"
    );
}

#[test]
fn a_peer_keeps_one_address_while_it_answers() {
    use std::collections::HashMap;
    use std::net::SocketAddr;

    let loopback: SocketAddr = "127.0.0.1:9010".parse().unwrap();
    let lan: SocketAddr = "192.168.1.39:9010".parse().unwrap();
    let mut known = HashMap::new();

    // Première annonce : adresse adoptée.
    assert!(super::adopt_addr(&mut known, "alice", loopback, 1_000));
    // La même annonce arrivée par l'autre canal (multicast/broadcast) ne doit
    // pas faire basculer l'adresse — c'est ce va-et-vient qui rouvrait une
    // connexion toutes les trois secondes.
    assert!(!super::adopt_addr(&mut known, "alice", lan, 1_000));
    assert!(!super::adopt_addr(&mut known, "alice", loopback, 1_003));
    assert!(!super::adopt_addr(&mut known, "alice", lan, 1_003));
    assert_eq!(known["alice"].0, loopback);

    // Vrai déménagement : l'adresse retenue s'est tue, la nouvelle est adoptée.
    let later = 1_003 + super::DISCOVERY_TIMEOUT;
    assert!(super::adopt_addr(&mut known, "alice", lan, later));
    assert_eq!(known["alice"].0, lan);
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
