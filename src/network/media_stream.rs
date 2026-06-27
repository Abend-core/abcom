//! Streaming des médias par morceaux (disque à disque, sans charger le fichier
//! en mémoire), ce qui vaut aussi bien pour quelques Ko que pour plusieurs Go.
//!
//! Protocole : l'émetteur se connecte au port média du destinataire
//! (`chat_port + 1`), envoie un en-tête (`u32` de longueur + JSON
//! [`MediaStreamHeader`]) puis les octets du fichier. Pour les médias > 1 Go,
//! l'en-tête porte `requires_ack` et le destinataire renvoie un octet
//! d'acceptation (1) ou de refus (0) avant toute écriture.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

use crate::config;
use crate::message::{AppEvent, MediaProgress, MediaSendJob, MediaStreamHeader, MediaStreamOffer};

const BUFFER_SIZE: usize = 64 * 1024;
const MAX_HEADER_BYTES: usize = 1024 * 1024;
const DECISION_TIMEOUT: Duration = Duration::from_secs(120);

fn to_io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

fn progress(id: &str, done: u64, total: u64, finished: bool) -> AppEvent {
    AppEvent::MediaProgressed(MediaProgress {
        id: id.to_string(),
        done,
        total,
        waiting: false,
        finished,
        failed: false,
    })
}

/// En attente de l'acceptation du destinataire (média > 1 Go, côté émetteur).
fn waiting(id: &str, total: u64) -> AppEvent {
    AppEvent::MediaProgressed(MediaProgress {
        id: id.to_string(),
        done: 0,
        total,
        waiting: true,
        finished: false,
        failed: false,
    })
}

fn failed(id: &str, total: u64) -> AppEvent {
    AppEvent::MediaProgressed(MediaProgress {
        id: id.to_string(),
        done: 0,
        total,
        waiting: false,
        finished: false,
        failed: true,
    })
}

/// Émetteur : pour chaque tâche, streame le fichier vers le destinataire.
pub async fn run_media_sender(mut rx: Receiver<MediaSendJob>, event_tx: Sender<AppEvent>) {
    while let Some(job) = rx.recv().await {
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            let id = job.header.media.id.clone();
            let total = job.header.media.size_bytes;
            if let Err(e) = stream_out(&job, &event_tx).await {
                eprintln!("[media] envoi échoué ({}): {}", id, e);
                let _ = event_tx.send(failed(&id, total)).await;
            }
        });
    }
}

async fn stream_out(job: &MediaSendJob, event_tx: &Sender<AppEvent>) -> std::io::Result<()> {
    // Port média du destinataire = son port de chat + 1 (cf. config::media_port).
    let media_addr = SocketAddr::new(job.to_addr.ip(), job.to_addr.port() + 1);
    let id = &job.header.media.id;
    let total = job.header.media.size_bytes;

    let mut stream = TcpStream::connect(media_addr).await?;
    let header = serde_json::to_vec(&job.header).map_err(to_io)?;
    stream.write_u32(header.len() as u32).await?;
    stream.write_all(&header).await?;
    stream.flush().await?;

    if job.header.requires_ack {
        // Côté émetteur : « en attente d'envoi » tant que le destinataire n'a
        // pas répondu.
        let _ = event_tx.send(waiting(id, total)).await;
        let accepted = matches!(
            tokio::time::timeout(DECISION_TIMEOUT, stream.read_u8()).await,
            Ok(Ok(1))
        );
        if !accepted {
            // Refus (ou absence de réponse) : on annote le fil côté émetteur.
            let _ = event_tx
                .send(AppEvent::MediaDeclined(job.header.clone()))
                .await;
            return Ok(());
        }
    }

    let mut file = tokio::fs::File::open(&job.source_path).await?;
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut sent = 0u64;
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        stream.write_all(&buffer[..read]).await?;
        sent += read as u64;
        let _ = event_tx.send(progress(id, sent, total, false)).await;
    }
    stream.flush().await?;
    stream.shutdown().await?;
    let _ = event_tx.send(progress(id, total, total, true)).await;
    Ok(())
}

/// Serveur : reçoit les flux média entrants et les écrit dans `media/<id>`.
pub async fn run_media_server(
    event_tx: Sender<AppEvent>,
    offer_tx: Sender<MediaStreamOffer>,
    media_dir: PathBuf,
) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", config::media_port())).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("[media] erreur de bind: {}", e);
            return;
        }
    };

    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let event_tx = event_tx.clone();
            let offer_tx = offer_tx.clone();
            let media_dir = media_dir.clone();
            tokio::spawn(async move {
                if let Err(e) = stream_in(stream, event_tx, offer_tx, media_dir).await {
                    eprintln!("[media] réception échouée: {}", e);
                }
            });
        }
    }
}

async fn stream_in(
    mut stream: TcpStream,
    event_tx: Sender<AppEvent>,
    offer_tx: Sender<MediaStreamOffer>,
    media_dir: PathBuf,
) -> std::io::Result<()> {
    let header_len = stream.read_u32().await? as usize;
    if header_len == 0 || header_len > MAX_HEADER_BYTES {
        return Err(to_io("en-tête média de taille invalide"));
    }
    let mut header_bytes = vec![0u8; header_len];
    stream.read_exact(&mut header_bytes).await?;
    let header: MediaStreamHeader = serde_json::from_slice(&header_bytes).map_err(to_io)?;

    // Médias volumineux : demander l'accord avant d'écrire le moindre octet.
    if header.requires_ack {
        let (decision_tx, decision_rx) = oneshot::channel::<bool>();
        let offer = MediaStreamOffer {
            from: header.from.clone(),
            filename: header.media.filename.clone(),
            size_bytes: header.media.size_bytes,
            decision_tx,
        };
        if offer_tx.send(offer).await.is_err() {
            let _ = stream.write_u8(0).await;
            return Ok(());
        }
        let accepted = matches!(
            tokio::time::timeout(DECISION_TIMEOUT, decision_rx).await,
            Ok(Ok(true))
        );
        stream.write_u8(u8::from(accepted)).await?;
        stream.flush().await?;
        if !accepted {
            return Ok(());
        }
    }

    let id = header.media.id.clone();
    let total = header.media.size_bytes;
    let path = media_dir.join(&id);

    // Annonce le message à l'UI : la carte média apparaît avec sa progression.
    let _ = event_tx.send(AppEvent::MediaIncoming(header)).await;

    // En cas d'échec en cours de réception : on signale l'interruption (la carte
    // est retirée côté UI) et on supprime le fichier partiel.
    if let Err(e) = receive_body(&mut stream, &media_dir, &path, &id, total, &event_tx).await {
        let _ = event_tx.send(failed(&id, total)).await;
        let _ = tokio::fs::remove_file(&path).await;
        return Err(e);
    }
    Ok(())
}

async fn receive_body(
    stream: &mut TcpStream,
    media_dir: &std::path::Path,
    path: &std::path::Path,
    id: &str,
    total: u64,
    event_tx: &Sender<AppEvent>,
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(media_dir).await?;
    let mut file = tokio::fs::File::create(path).await?;
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut received = 0u64;
    while received < total {
        let to_read = std::cmp::min((total - received) as usize, buffer.len());
        let read = stream.read(&mut buffer[..to_read]).await?;
        if read == 0 {
            return Err(to_io("fin de flux média prématurée"));
        }
        file.write_all(&buffer[..read]).await?;
        received += read as u64;
        let _ = event_tx.send(progress(id, received, total, false)).await;
    }
    file.flush().await?;
    let _ = event_tx.send(progress(id, total, total, true)).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{MediaAttachment, MediaKind};
    use tokio::sync::mpsc;

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
        let server_tx = event_tx.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            stream_in(stream, server_tx, offer_tx, dir).await
        });

        let source = std::env::temp_dir().join(format!("abcom_ms_src_{}.bin", std::process::id()));
        let payload = vec![7u8; 200_000];
        std::fs::write(&source, &payload).unwrap();

        let job = MediaSendJob {
            to_addr: format!("127.0.0.1:{}", port - 1).parse().unwrap(),
            source_path: source.clone(),
            header: header("test.bin", payload.len() as u64, false),
        };

        stream_out(&job, &event_tx).await.unwrap();
        server.await.unwrap().unwrap();

        assert_eq!(std::fs::read(media_dir.join("test.bin")).unwrap(), payload);

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
    async fn large_media_streams_after_acceptance() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let media_dir = unique_dir("acc");

        let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(256);
        let (offer_tx, mut offer_rx) = mpsc::channel::<MediaStreamOffer>(4);

        // Dès qu'une offre arrive, on l'accepte.
        tokio::spawn(async move {
            if let Some(offer) = offer_rx.recv().await {
                let _ = offer.decision_tx.send(true);
            }
        });

        let dir = media_dir.clone();
        let server_tx = event_tx.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            stream_in(stream, server_tx, offer_tx, dir).await
        });

        let source = std::env::temp_dir().join(format!("abcom_acc_src_{}.bin", std::process::id()));
        let payload = vec![9u8; 100_000];
        std::fs::write(&source, &payload).unwrap();

        let job = MediaSendJob {
            to_addr: format!("127.0.0.1:{}", port - 1).parse().unwrap(),
            source_path: source.clone(),
            header: header("big.zip", payload.len() as u64, true),
        };

        stream_out(&job, &event_tx).await.unwrap();
        server.await.unwrap().unwrap();

        assert_eq!(std::fs::read(media_dir.join("big.zip")).unwrap(), payload);

        let (mut waiting_seen, mut finished) = (false, false);
        while let Ok(event) = event_rx.try_recv() {
            if let AppEvent::MediaProgressed(p) = event {
                waiting_seen |= p.waiting;
                finished |= p.finished;
            }
        }
        assert!(waiting_seen, "état « en attente » attendu avant acceptation");
        assert!(finished, "progression terminée attendue après acceptation");

        std::fs::remove_dir_all(&media_dir).ok();
        std::fs::remove_file(&source).ok();
    }

    #[tokio::test]
    async fn large_media_declined_writes_nothing() {
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
        let server_tx = event_tx.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            stream_in(stream, server_tx, offer_tx, dir).await
        });

        let source = std::env::temp_dir().join(format!("abcom_dec_src_{}.bin", std::process::id()));
        std::fs::write(&source, vec![1u8; 50_000]).unwrap();

        let job = MediaSendJob {
            to_addr: format!("127.0.0.1:{}", port - 1).parse().unwrap(),
            source_path: source.clone(),
            header: header("refuse.zip", 50_000, true),
        };

        stream_out(&job, &event_tx).await.unwrap();
        server.await.unwrap().unwrap();

        // Rien n'a été écrit, et l'émetteur reçoit un refus (pas de réception).
        assert!(!media_dir.join("refuse.zip").exists());
        let (mut declined, mut incoming) = (false, false);
        while let Ok(event) = event_rx.try_recv() {
            match event {
                AppEvent::MediaDeclined(_) => declined = true,
                AppEvent::MediaIncoming(_) => incoming = true,
                _ => {}
            }
        }
        assert!(declined, "refus attendu côté émetteur");
        assert!(!incoming, "aucune réception ne doit démarrer");

        std::fs::remove_dir_all(&media_dir).ok();
        std::fs::remove_file(&source).ok();
    }
}
