use std::sync::Arc;

use super::*;
use crate::identity::Identity;
use crate::message::{MediaAttachment, MediaKind};
use crate::network::secure::TrustStore;
use crate::protocol::{MAX_MEDIA_TRANSFER_BYTES, MEDIA_ACK_THRESHOLD_BYTES};
use tokio::sync::mpsc;

fn test_ctx(username: &str, tx: mpsc::Sender<AppEvent>) -> Arc<NetContext> {
    Arc::new(NetContext {
        identity: Identity::ephemeral().unwrap(),
        username: username.to_string(),
        trust: Arc::new(TrustStore::new(Default::default(), None)),
        event_tx: tx,
        psk: None,
    })
}

fn unique_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "abcom_ms_{}_{}_{:?}",
        tag,
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn header(id: &str, size: u64, requires_ack: bool) -> MediaStreamHeader {
    MediaStreamHeader {
        from: "bob".to_string(),
        to_user: Some("ellis".to_string()),
        timestamp: "12:00".to_string(),
        timestamp_epoch: None,
        media: MediaAttachment {
            id: id.to_string(),
            filename: id.to_string(),
            kind: MediaKind::File,
            size_bytes: size,
            url: None,
            width: None,
            height: None,
        },
        requires_ack,
    }
}

#[tokio::test]
async fn streams_a_file_end_to_end() {
    // Serveur sur un port éphémère ; l'émetteur vise `port - 1` (il ajoute +1).
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let media_dir = unique_dir("e2e");

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(256);
    let (offer_tx, _offer_rx) = mpsc::channel::<MediaStreamOffer>(4);

    let dir = media_dir.clone();
    let server_ctx = test_ctx("ellis", event_tx.clone());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        stream_in(stream, server_ctx, offer_tx, dir).await
    });

    let source = std::env::temp_dir().join(format!("abcom_ms_src_{}.bin", std::process::id()));
    let payload = vec![7u8; 200_000];
    std::fs::write(&source, &payload).unwrap();

    let job = MediaSendJob {
        to_peer: "ellis".into(),
        to_addr: format!("127.0.0.1:{}", port - 1).parse().unwrap(),
        source_path: source.clone(),
        header: header("test.bin", payload.len() as u64, false),
    };

    let client_ctx = test_ctx("bob", event_tx.clone());
    stream_out(&job, &client_ctx).await.unwrap();
    server.await.unwrap().unwrap();

    assert_eq!(std::fs::read(media_dir.join("test.bin")).unwrap(), payload);
    assert!(
        std::fs::read_dir(&media_dir).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part")),
        "aucun fichier temporaire ne doit rester après succès"
    );

    let (mut incoming, mut finished) = (false, false);
    while let Ok(event) = event_rx.try_recv() {
        match event {
            AppEvent::MediaIncoming(_) => incoming = true,
            AppEvent::MediaProgressed(p) if p.finished => finished = true,
            _ => {}
        }
    }
    assert!(incoming, "MediaIncoming attendu");
    assert!(finished, "progression terminée attendue");

    std::fs::remove_dir_all(&media_dir).ok();
    std::fs::remove_file(&source).ok();
}

#[tokio::test]
async fn forged_requires_ack_is_ignored_for_small_media() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let media_dir = unique_dir("acc");

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(256);
    let (offer_tx, mut offer_rx) = mpsc::channel::<MediaStreamOffer>(4);

    let dir = media_dir.clone();
    let server_ctx = test_ctx("ellis", event_tx.clone());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        stream_in(stream, server_ctx, offer_tx, dir).await
    });

    let source = std::env::temp_dir().join(format!("abcom_acc_src_{}.bin", std::process::id()));
    let payload = vec![9u8; 100_000];
    std::fs::write(&source, &payload).unwrap();

    let job = MediaSendJob {
        to_peer: "ellis".into(),
        to_addr: format!("127.0.0.1:{}", port - 1).parse().unwrap(),
        source_path: source.clone(),
        header: header("big.zip", payload.len() as u64, true),
    };

    let client_ctx = test_ctx("bob", event_tx.clone());
    stream_out(&job, &client_ctx).await.unwrap();
    server.await.unwrap().unwrap();

    assert_eq!(std::fs::read(media_dir.join("big.zip")).unwrap(), payload);

    let (mut waiting_seen, mut finished) = (false, false);
    while let Ok(event) = event_rx.try_recv() {
        if let AppEvent::MediaProgressed(p) = event {
            waiting_seen |= p.waiting;
            finished |= p.finished;
        }
    }
    assert!(
        !waiting_seen,
        "le booléen falsifié ne doit pas imposer d'accord"
    );
    assert!(
        offer_rx.try_recv().is_err(),
        "aucune offre ne doit être créée"
    );
    assert!(finished, "progression terminée attendue");

    std::fs::remove_dir_all(&media_dir).ok();
    std::fs::remove_file(&source).ok();
}

#[tokio::test]
async fn forged_missing_ack_is_recomputed_and_declined() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let media_dir = unique_dir("dec");

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(256);
    let (offer_tx, mut offer_rx) = mpsc::channel::<MediaStreamOffer>(4);

    // On refuse l'offre.
    tokio::spawn(async move {
        if let Some(offer) = offer_rx.recv().await {
            let _ = offer.decision_tx.send(false);
        }
    });

    let dir = media_dir.clone();
    let server_ctx = test_ctx("ellis", event_tx.clone());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        stream_in(stream, server_ctx, offer_tx, dir).await
    });

    let source = std::env::temp_dir().join(format!("abcom_dec_src_{}.bin", std::process::id()));
    let announced_size = MEDIA_ACK_THRESHOLD_BYTES + 1;
    std::fs::File::create(&source)
        .unwrap()
        .set_len(announced_size)
        .unwrap();

    let job = MediaSendJob {
        to_peer: "ellis".into(),
        to_addr: format!("127.0.0.1:{}", port - 1).parse().unwrap(),
        source_path: source.clone(),
        header: header("refuse.zip", announced_size, false),
    };

    let client_ctx = test_ctx("bob", event_tx.clone());
    stream_out(&job, &client_ctx).await.unwrap();
    server.await.unwrap().unwrap();

    // Rien n'a été écrit, et l'émetteur reçoit un refus (pas de réception).
    assert!(!media_dir.join("refuse.zip").exists());
    let (mut declined, mut incoming) = (false, false);
    while let Ok(event) = event_rx.try_recv() {
        match event {
            AppEvent::MediaDeclined { .. } => declined = true,
            AppEvent::MediaIncoming(_) => incoming = true,
            _ => {}
        }
    }
    assert!(declined, "refus attendu côté émetteur");
    assert!(!incoming, "aucune réception ne doit démarrer");

    std::fs::remove_dir_all(&media_dir).ok();
    std::fs::remove_file(&source).ok();
}

#[test]
fn safe_media_id_accepts_normal_rejects_traversal() {
    assert!(is_safe_media_id("1720000000-photo.png"));
    assert!(is_safe_media_id("file_with-dots.tar.gz"));
    assert!(!is_safe_media_id("../secret"));
    assert!(!is_safe_media_id("a/b"));
    assert!(!is_safe_media_id("/etc/passwd"));
    assert!(!is_safe_media_id(".."));
    assert!(!is_safe_media_id("."));
    assert!(!is_safe_media_id(""));
}

#[test]
fn incoming_header_must_match_authenticated_peer_and_local_target() {
    let mut h = header("ok.bin", 10, false);
    assert!(!validate_incoming_header(&h, "bob", "ellis").unwrap());

    h.from = "mallory".to_string();
    assert!(validate_incoming_header(&h, "bob", "ellis").is_err());

    h.from = "bob".to_string();
    h.to_user = Some("carol".to_string());
    assert!(validate_incoming_header(&h, "bob", "ellis").is_err());

    h.to_user = Some("#equipe".to_string());
    assert!(validate_incoming_header(&h, "bob", "ellis").is_ok());
}

#[test]
fn incoming_header_enforces_absolute_transfer_limit() {
    let mut h = header("huge.bin", MAX_MEDIA_TRANSFER_BYTES, false);
    assert!(validate_incoming_header(&h, "bob", "ellis").is_ok());

    h.media.size_bytes += 1;
    assert!(validate_incoming_header(&h, "bob", "ellis").is_err());
}

#[test]
fn chunk_cannot_exceed_remaining_body() {
    assert!(validate_chunk_len(4, 4).is_ok());
    assert!(validate_chunk_len(5, 4).is_err());
}

#[tokio::test]
async fn stalled_phase_times_out() {
    let result = io_timeout(
        std::time::Duration::from_millis(10),
        "le test",
        std::future::pending::<std::io::Result<()>>(),
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("timeout"));
}

#[tokio::test]
async fn completed_media_never_overwrites_an_existing_id() {
    let dir = unique_dir("no-overwrite");
    std::fs::create_dir_all(&dir).unwrap();
    let temp = dir.join("incoming.part");
    let final_path = dir.join("same-id.bin");
    std::fs::write(&temp, b"new").unwrap();
    std::fs::write(&final_path, b"original").unwrap();

    assert!(rename_completed(&temp, &final_path).await.is_err());
    assert_eq!(std::fs::read(&final_path).unwrap(), b"original");

    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn rejects_path_traversal_id_and_writes_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let media_dir = unique_dir("trav");

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(256);
    let (offer_tx, _offer_rx) = mpsc::channel::<MediaStreamOffer>(4);

    let dir = media_dir.clone();
    let server_ctx = test_ctx("ellis", event_tx.clone());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        stream_in(stream, server_ctx, offer_tx, dir).await
    });

    let source = std::env::temp_dir().join(format!("abcom_trav_src_{}.bin", std::process::id()));
    std::fs::write(&source, vec![7u8; 20_000]).unwrap();

    // Un pair malveillant annonce un id qui tenterait de sortir de `media/`.
    let job = MediaSendJob {
        to_peer: "ellis".into(),
        to_addr: format!("127.0.0.1:{}", port - 1).parse().unwrap(),
        source_path: source.clone(),
        header: header("../../../../tmp/abcom_evil", 20_000, false),
    };

    let client_ctx = test_ctx("bob", event_tx.clone());
    let _ = stream_out(&job, &client_ctx).await; // peut échouer : le serveur coupe

    // Le serveur rejette l'en-tête avant toute écriture : erreur, aucun dossier
    // `media/` créé, aucune réception annoncée à l'UI.
    assert!(
        server.await.unwrap().is_err(),
        "un id de path traversal doit être rejeté"
    );
    assert!(!media_dir.exists(), "aucun fichier ne doit être écrit");
    let mut incoming = false;
    while let Ok(event) = event_rx.try_recv() {
        if let AppEvent::MediaIncoming(_) = event {
            incoming = true;
        }
    }
    assert!(!incoming, "aucune réception ne doit démarrer");

    std::fs::remove_dir_all(&media_dir).ok();
    std::fs::remove_file(&source).ok();
}
