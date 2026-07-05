use super::*;
use crate::identity::Identity;
use tokio::net::{TcpListener, TcpStream};

/// Paire de flux chiffrés connectés (initiateur, répondeur) après handshake.
async fn secure_pair() -> (SecureStream, SecureStream, Vec<u8>, Vec<u8>) {
    let alice = Identity::ephemeral().unwrap();
    let bob = Identity::ephemeral().unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let bob_task = {
        let bob = bob.clone();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (transport, remote) = handshake_responder(&mut stream, &bob, None).await.unwrap();
            (SecureStream::new(stream, transport), remote)
        })
    };

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (transport, bob_key) = handshake_initiator(&mut stream, &alice, None)
        .await
        .unwrap();
    let alice_stream = SecureStream::new(stream, transport);
    let (bob_stream, alice_key) = bob_task.await.unwrap();

    assert_eq!(bob_key, bob.public, "clé statique du répondeur");
    assert_eq!(alice_key, alice.public, "clé statique de l'initiateur");
    (alice_stream, bob_stream, alice.public, bob.public)
}

#[tokio::test]
async fn round_trip_small_message() {
    let (mut alice, mut bob, _, _) = secure_pair().await;
    alice.send(b"bonjour bob").await.unwrap();
    assert_eq!(bob.recv().await.unwrap(), b"bonjour bob");
    bob.send(b"bonjour alice").await.unwrap();
    assert_eq!(alice.recv().await.unwrap(), b"bonjour alice");
}

#[tokio::test]
async fn round_trip_large_message_spans_frames() {
    // > 65 519 octets : le message logique s'étale sur plusieurs frames
    // Noise (cas des annonces d'avatar, qui échouaient en clair à 64 Ko).
    let (mut alice, mut bob, _, _) = secure_pair().await;
    let big: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    alice.send(&big).await.unwrap();
    assert_eq!(bob.recv().await.unwrap(), big);
}

#[tokio::test]
async fn hello_exchange_identifies_both_sides() {
    let (mut alice, mut bob, _, _) = secure_pair().await;
    let (from_bob, from_alice) = tokio::join!(
        exchange_hello(&mut alice, "alice", true),
        exchange_hello(&mut bob, "bob", false),
    );
    assert_eq!(from_bob.unwrap(), "bob");
    assert_eq!(from_alice.unwrap(), "alice");
}

#[tokio::test]
async fn psk_handshake_succeeds_with_shared_passphrase() {
    let alice = Identity::ephemeral().unwrap();
    let bob = Identity::ephemeral().unwrap();
    let psk = derive_psk("salon-secret");
    assert_eq!(psk.len(), 32);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let bob_psk = psk.clone();
    let bob_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (transport, _) = handshake_responder(&mut stream, &bob, Some(&bob_psk))
            .await
            .unwrap();
        let mut secure = SecureStream::new(stream, transport);
        secure.recv().await.unwrap()
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (transport, _) = handshake_initiator(&mut stream, &alice, Some(&psk))
        .await
        .unwrap();
    let mut secure = SecureStream::new(stream, transport);
    secure.send(b"dans le salon").await.unwrap();
    assert_eq!(bob_task.await.unwrap(), b"dans le salon");
}

#[tokio::test]
async fn psk_handshake_fails_without_passphrase() {
    // Un client sans passphrase ne peut pas rejoindre un serveur protégé.
    let alice = Identity::ephemeral().unwrap();
    let bob = Identity::ephemeral().unwrap();
    let psk = derive_psk("salon-secret");

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let bob_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        handshake_responder(&mut stream, &bob, Some(&psk)).await
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let client = handshake_initiator(&mut stream, &alice, None).await;
    let server = bob_task.await.unwrap();
    assert!(
        client.is_err() || server.is_err(),
        "le handshake doit échouer sans la passphrase"
    );
}

#[tokio::test]
async fn psk_handshake_fails_with_wrong_passphrase() {
    let alice = Identity::ephemeral().unwrap();
    let bob = Identity::ephemeral().unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let bob_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let psk = derive_psk("bonne-passphrase");
        handshake_responder(&mut stream, &bob, Some(&psk)).await
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let wrong = derive_psk("mauvaise-passphrase");
    let client = handshake_initiator(&mut stream, &alice, Some(&wrong)).await;
    let server = bob_task.await.unwrap();
    assert!(client.is_err() || server.is_err());
}

#[test]
fn trust_store_pins_then_verifies() {
    let store = TrustStore::new(Default::default(), None);
    let key_a = vec![1u8; 32];
    let key_b = vec![2u8; 32];

    assert_eq!(store.verify_and_pin("bob", &key_a), Trust::Pinned);
    assert_eq!(store.verify_and_pin("bob", &key_a), Trust::Match);
    assert_eq!(store.verify_and_pin("bob", &key_b), Trust::Mismatch);
    // L'épinglage d'origine n'est pas écrasé par la tentative refusée.
    assert_eq!(store.verify_and_pin("bob", &key_a), Trust::Match);
}

#[test]
fn trust_store_loads_preexisting_pins() {
    let mut keys = std::collections::HashMap::new();
    keys.insert("alice".to_string(), vec![7u8; 32]);
    let store = TrustStore::new(keys, None);
    assert_eq!(store.verify_and_pin("alice", &[7u8; 32]), Trust::Match);
    assert_eq!(store.verify_and_pin("alice", &[8u8; 32]), Trust::Mismatch);
}
