//! Streaming des médias par morceaux (disque à disque, sans charger le
//! fichier en mémoire), sur une connexion **chiffrée Noise XX** dédiée au
//! transfert.
//!
//! Protocole : l'émetteur se connecte au port média du destinataire
//! (`chat_port + 1`), fait le handshake Noise + échange Hello (TOFU), envoie
//! l'en-tête JSON [`MediaStreamHeader`] puis les chunks du fichier en
//! messages chiffrés. Pour les médias > 1 Go, l'en-tête porte `requires_ack`
//! et le destinataire renvoie un octet d'acceptation (1) ou de refus (0)
//! avant tout envoi de données.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

use crate::config;
use crate::message::{AppEvent, MediaProgress, MediaSendJob, MediaStreamHeader, MediaStreamOffer};

use super::secure::{exchange_hello, handshake_initiator, handshake_responder, SecureStream, Trust};
use super::NetContext;

/// Taille d'un chunk de fichier : tient dans un seul message Noise
/// (65 535 octets max, tag AEAD et en-tête de longueur déduits).
const BUFFER_SIZE: usize = 60 * 1024;
const DECISION_TIMEOUT: Duration = Duration::from_secs(120);
/// Intervalle minimal entre deux événements de progression vers l'UI.
/// Sans ce throttle, un événement partait par chunk (~17 000/Go) : le canal
/// mpsc (256 places) saturait et le `send().await` mettait le transfert
/// lui-même en attente de la boucle de rendu.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

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

/// Émetteur de progression throttlé : relaie au plus un événement par
/// [`PROGRESS_INTERVAL`], plus systématiquement le dernier (fin de transfert).
struct ProgressReporter {
    last_emit: Option<std::time::Instant>,
}

impl ProgressReporter {
    fn new() -> Self {
        Self { last_emit: None }
    }

    async fn report(
        &mut self,
        tx: &Sender<AppEvent>,
        id: &str,
        done: u64,
        total: u64,
        finished: bool,
    ) {
        let now = std::time::Instant::now();
        let due = match self.last_emit {
            None => true,
            Some(prev) => now.duration_since(prev) >= PROGRESS_INTERVAL,
        };
        if finished || due {
            self.last_emit = Some(now);
            let _ = tx.send(progress(id, done, total, finished)).await;
        }
    }
}

/// Émetteur : pour chaque tâche, streame le fichier vers le destinataire.
pub async fn run_media_sender(mut rx: Receiver<MediaSendJob>, ctx: Arc<NetContext>) {
    while let Some(job) = rx.recv().await {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let id = job.header.media.id.clone();
            let total = job.header.media.size_bytes;
            if let Err(e) = stream_out(&job, &ctx).await {
                eprintln!("[media] envoi échoué ({}): {}", id, e);
                let _ = ctx.event_tx.send(failed(&id, total)).await;
            }
        });
    }
}

/// Établit la connexion média chiffrée vers `addr` (handshake + Hello + TOFU).
async fn connect_secure(
    addr: std::net::SocketAddr,
    ctx: &NetContext,
) -> std::io::Result<SecureStream> {
    let mut stream = TcpStream::connect(addr).await?;
    let (transport, remote_key) = handshake_initiator(&mut stream, &ctx.identity).await?;
    let mut secure = SecureStream::new(stream, transport);
    let peer = exchange_hello(&mut secure, &ctx.username, true).await?;
    if ctx.trust.verify_and_pin(&peer, &remote_key) == Trust::Mismatch {
        ctx.report_key_mismatch(&peer).await;
        return Err(to_io("clé du pair inattendue (TOFU)"));
    }
    Ok(secure)
}

async fn stream_out(job: &MediaSendJob, ctx: &NetContext) -> std::io::Result<()> {
    // Port média du destinataire = son port de chat + 1 (cf. config::media_port).
    let media_addr = std::net::SocketAddr::new(job.to_addr.ip(), job.to_addr.port() + 1);
    let id = &job.header.media.id;
    let total = job.header.media.size_bytes;
    let event_tx = &ctx.event_tx;

    let mut secure = connect_secure(media_addr, ctx).await?;
    let header = serde_json::to_vec(&job.header).map_err(to_io)?;
    secure.send(&header).await?;

    if job.header.requires_ack {
        // Côté émetteur : « en attente d'envoi » tant que le destinataire n'a
        // pas répondu.
        let _ = event_tx.send(waiting(id, total)).await;
        let accepted = matches!(
            tokio::time::timeout(DECISION_TIMEOUT, secure.recv()).await,
            Ok(Ok(reply)) if reply == [1]
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
    let mut reporter = ProgressReporter::new();
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        secure.send(&buffer[..read]).await?;
        sent += read as u64;
        reporter.report(event_tx, id, sent, total, false).await;
    }
    reporter.report(event_tx, id, total, total, true).await;
    Ok(())
}

/// Serveur : reçoit les flux média entrants (chiffrés) et les écrit dans
/// `media/<id>`.
pub async fn run_media_server(
    ctx: Arc<NetContext>,
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
            let ctx = ctx.clone();
            let offer_tx = offer_tx.clone();
            let media_dir = media_dir.clone();
            tokio::spawn(async move {
                if let Err(e) = stream_in(stream, ctx, offer_tx, media_dir).await {
                    eprintln!("[media] réception échouée: {}", e);
                }
            });
        }
    }
}

async fn stream_in(
    mut stream: TcpStream,
    ctx: Arc<NetContext>,
    offer_tx: Sender<MediaStreamOffer>,
    media_dir: PathBuf,
) -> std::io::Result<()> {
    let (transport, remote_key) = handshake_responder(&mut stream, &ctx.identity).await?;
    let mut secure = SecureStream::new(stream, transport);
    let peer = exchange_hello(&mut secure, &ctx.username, false).await?;
    if ctx.trust.verify_and_pin(&peer, &remote_key) == Trust::Mismatch {
        ctx.report_key_mismatch(&peer).await;
        return Err(to_io("clé du pair inattendue (TOFU)"));
    }

    let header_bytes = secure.recv().await?;
    let header: MediaStreamHeader = serde_json::from_slice(&header_bytes).map_err(to_io)?;
    let event_tx = ctx.event_tx.clone();

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
            let _ = secure.send(&[0]).await;
            return Ok(());
        }
        let accepted = matches!(
            tokio::time::timeout(DECISION_TIMEOUT, decision_rx).await,
            Ok(Ok(true))
        );
        secure.send(&[u8::from(accepted)]).await?;
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
    if let Err(e) = receive_body(&mut secure, &media_dir, &path, &id, total, &event_tx).await {
        let _ = event_tx.send(failed(&id, total)).await;
        let _ = tokio::fs::remove_file(&path).await;
        return Err(e);
    }
    Ok(())
}

async fn receive_body(
    secure: &mut SecureStream,
    media_dir: &std::path::Path,
    path: &std::path::Path,
    id: &str,
    total: u64,
    event_tx: &Sender<AppEvent>,
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(media_dir).await?;
    let mut file = tokio::fs::File::create(path).await?;
    let mut received = 0u64;
    let mut reporter = ProgressReporter::new();
    while received < total {
        let chunk = secure.recv().await?;
        if chunk.is_empty() {
            return Err(to_io("fin de flux média prématurée"));
        }
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        reporter.report(event_tx, id, received, total, false).await;
    }
    file.flush().await?;
    reporter.report(event_tx, id, total, total, true).await;
    Ok(())
}

#[cfg(test)]
#[path = "../tests/test_network_media_stream.rs"]
mod tests;
