//! Streaming des médias par morceaux (disque à disque, sans charger le
//! fichier en mémoire), sur une connexion **chiffrée Noise XX** dédiée au
//! transfert.
//!
//! Protocole : l'émetteur se connecte au port média du destinataire
//! (`chat_port + 1`), fait le handshake Noise + échange Hello (TOFU), envoie
//! l'en-tête JSON [`MediaStreamHeader`] puis les chunks du fichier en
//! messages chiffrés. Au-delà du seuil partagé du protocole, le destinataire
//! renvoie un octet d'acceptation (1) ou de refus (0) avant tout envoi de
//! données. Cette décision est recalculée depuis la taille annoncée.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio::sync::Semaphore;

use crate::config;
use crate::message::{AppEvent, MediaProgress, MediaSendJob, MediaStreamHeader, MediaStreamOffer};
use crate::protocol::{media_requires_ack, MAX_MEDIA_TRANSFER_BYTES};

use super::secure::{
    exchange_hello, handshake_initiator, handshake_responder, SecureStream, Trust,
};
use super::NetContext;

/// Taille d'un chunk de fichier : tient dans un seul message Noise
/// (65 535 octets max, tag AEAD et en-tête de longueur déduits).
const BUFFER_SIZE: usize = 60 * 1024;
const DECISION_TIMEOUT: Duration = Duration::from_secs(120);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
const BODY_CHUNK_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONCURRENT_RECEIVES: usize = 4;
/// Intervalle minimal entre deux événements de progression vers l'UI.
/// Sans ce throttle, un événement partait par chunk (~17 000/Go) : le canal
/// mpsc (256 places) saturait et le `send().await` mettait le transfert
/// lui-même en attente de la boucle de rendu.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

fn to_io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

async fn io_timeout<T>(
    duration: Duration,
    phase: &str,
    future: impl std::future::Future<Output = std::io::Result<T>>,
) -> std::io::Result<T> {
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| to_io(format!("timeout pendant {phase}")))?
}

/// Vrai si `id` est un nom de fichier sûr : un seul composant de chemin, donc
/// utilisable tel quel sous `media/` sans risque de path traversal. L'`id`
/// arrivant du réseau, on rejette tout ce qui contient un séparateur ou vaut
/// `.`/`..` (`Path::file_name` renvoie alors autre chose que l'`id` entier).
fn is_safe_media_id(id: &str) -> bool {
    std::path::Path::new(id).file_name() == Some(std::ffi::OsStr::new(id))
}

fn is_valid_media_target(to_user: Option<&str>, local_username: &str) -> bool {
    match to_user {
        // `None` désigne la conversation publique « Tous ».
        None => true,
        Some(target) if target == local_username => true,
        Some(target) => target.starts_with('#') && target.len() > 1,
    }
}

fn validate_incoming_header(
    header: &MediaStreamHeader,
    authenticated_peer: &str,
    local_username: &str,
) -> std::io::Result<bool> {
    if header.from != authenticated_peer {
        return Err(to_io("émetteur du média différent du pair authentifié"));
    }
    if !is_valid_media_target(header.to_user.as_deref(), local_username) {
        return Err(to_io("destinataire du média invalide"));
    }
    if !is_safe_media_id(&header.media.id) {
        return Err(to_io("identifiant de média invalide"));
    }
    if header.media.size_bytes > MAX_MEDIA_TRANSFER_BYTES {
        return Err(to_io("média trop volumineux (maximum 2 Gio)"));
    }
    Ok(media_requires_ack(header.media.size_bytes))
}

fn validate_chunk_len(chunk_len: usize, remaining: u64) -> std::io::Result<()> {
    if chunk_len as u64 > remaining {
        return Err(to_io("chunk média plus grand que le reste attendu"));
    }
    Ok(())
}

fn progress(id: &str, done: u64, total: u64, finished: bool, outgoing: bool) -> AppEvent {
    AppEvent::MediaProgressed(MediaProgress {
        id: id.to_string(),
        done,
        total,
        waiting: false,
        finished,
        failed: false,
        outgoing,
    })
}

/// En attente de l'acceptation du destinataire (média au-delà du seuil d'accord, côté émetteur).
fn waiting(id: &str, total: u64) -> AppEvent {
    AppEvent::MediaProgressed(MediaProgress {
        id: id.to_string(),
        done: 0,
        total,
        waiting: true,
        finished: false,
        failed: false,
        outgoing: true,
    })
}

fn failed(id: &str, total: u64, outgoing: bool) -> AppEvent {
    AppEvent::MediaProgressed(MediaProgress {
        id: id.to_string(),
        done: 0,
        total,
        waiting: false,
        finished: false,
        failed: true,
        outgoing,
    })
}

/// Émetteur de progression throttlé : relaie au plus un événement par
/// [`PROGRESS_INTERVAL`], plus systématiquement le dernier (fin de transfert).
struct ProgressReporter {
    last_emit: Option<std::time::Instant>,
    outgoing: bool,
}

impl ProgressReporter {
    fn new(outgoing: bool) -> Self {
        Self {
            last_emit: None,
            outgoing,
        }
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
            let _ = tx
                .send(progress(id, done, total, finished, self.outgoing))
                .await;
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
                tracing::warn!("envoi échoué ({}): {}", id, e);
                let _ = ctx.event_tx.send(failed(&id, total, true)).await;
            }
        });
    }
}

/// Établit la connexion média chiffrée vers `addr` (handshake + Hello + TOFU).
async fn connect_secure(
    expected_peer: &str,
    addr: std::net::SocketAddr,
    ctx: &NetContext,
) -> std::io::Result<SecureStream> {
    let mut stream = io_timeout(
        HANDSHAKE_TIMEOUT,
        "la connexion média",
        TcpStream::connect(addr),
    )
    .await?;
    let (transport, remote_key) = io_timeout(
        HANDSHAKE_TIMEOUT,
        "le handshake média",
        handshake_initiator(&mut stream, &ctx.identity, ctx.psk_bytes()),
    )
    .await?;
    let mut secure = SecureStream::new(stream, transport);
    let peer = io_timeout(
        HELLO_TIMEOUT,
        "l'échange Hello média",
        exchange_hello(&mut secure, &ctx.username, true),
    )
    .await?;
    if peer != expected_peer {
        return Err(to_io(format!(
            "identité média inattendue : {peer} au lieu de {expected_peer}"
        )));
    }
    if ctx.trust.verify_and_pin(&peer, &remote_key) == Trust::Mismatch {
        ctx.report_key_mismatch(&peer, &remote_key).await;
        return Err(to_io("clé du pair inattendue (TOFU)"));
    }
    Ok(secure)
}

async fn stream_out(job: &MediaSendJob, ctx: &NetContext) -> std::io::Result<()> {
    // Port média du destinataire = son port de chat + 1 (cf. config::media_port).
    let media_port = job
        .to_addr
        .port()
        .checked_add(1)
        .ok_or_else(|| to_io("port média invalide"))?;
    let media_addr = std::net::SocketAddr::new(job.to_addr.ip(), media_port);
    let id = &job.header.media.id;
    let total = job.header.media.size_bytes;
    let event_tx = &ctx.event_tx;

    if total > MAX_MEDIA_TRANSFER_BYTES {
        return Err(to_io("média trop volumineux (maximum 2 Gio)"));
    }
    if tokio::fs::metadata(&job.source_path).await?.len() != total {
        return Err(to_io("taille du fichier source différente de l'en-tête"));
    }

    let mut secure = connect_secure(&job.to_peer, media_addr, ctx).await?;
    let header = serde_json::to_vec(&job.header).map_err(to_io)?;
    io_timeout(
        HEADER_TIMEOUT,
        "l'envoi de l'en-tête média",
        secure.send(&header),
    )
    .await?;

    let awaits_decision = media_requires_ack(total)
        || job
            .header
            .to_user
            .as_deref()
            .is_some_and(|to| to.starts_with('#'));
    if awaits_decision {
        if media_requires_ack(total) {
            // Côté émetteur : « en attente d'envoi » tant que le destinataire
            // n'a pas répondu.
            let _ = event_tx.send(waiting(id, total)).await;
        }
        let accepted = matches!(
            tokio::time::timeout(DECISION_TIMEOUT, secure.recv()).await,
            Ok(Ok(reply)) if reply == [1]
        );
        if !accepted {
            // Refus (ou absence de réponse) : on annote le fil côté émetteur.
            let _ = event_tx
                .send(AppEvent::MediaDeclined {
                    peer: job.to_peer.clone(),
                    header: job.header.clone(),
                })
                .await;
            return Ok(());
        }
    }

    let mut file = tokio::fs::File::open(&job.source_path).await?;
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut sent = 0u64;
    let mut reporter = ProgressReporter::new(true);
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        validate_chunk_len(read, total - sent)?;
        io_timeout(
            BODY_CHUNK_TIMEOUT,
            "l'envoi du corps média",
            secure.send(&buffer[..read]),
        )
        .await?;
        sent += read as u64;
        reporter.report(event_tx, id, sent, total, false).await;
    }
    if sent != total {
        return Err(to_io("fin du fichier source avant la taille annoncée"));
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
            tracing::error!("erreur de bind : {}", e);
            return;
        }
    };

    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_RECEIVES));
    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let Ok(permit) = permits.clone().try_acquire_owned() else {
                tracing::warn!(
                    "réception média refusée : limite de {MAX_CONCURRENT_RECEIVES} atteinte"
                );
                continue;
            };
            let ctx = ctx.clone();
            let offer_tx = offer_tx.clone();
            let media_dir = media_dir.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) = stream_in(stream, ctx, offer_tx, media_dir).await {
                    tracing::warn!("réception échouée : {}", e);
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
    let (transport, remote_key) = io_timeout(
        HANDSHAKE_TIMEOUT,
        "le handshake média",
        handshake_responder(&mut stream, &ctx.identity, ctx.psk_bytes()),
    )
    .await?;
    let mut secure = SecureStream::new(stream, transport);
    let peer = io_timeout(
        HELLO_TIMEOUT,
        "l'échange Hello média",
        exchange_hello(&mut secure, &ctx.username, false),
    )
    .await?;
    if ctx.trust.verify_and_pin(&peer, &remote_key) == Trust::Mismatch {
        ctx.report_key_mismatch(&peer, &remote_key).await;
        return Err(to_io("clé du pair inattendue (TOFU)"));
    }

    let header_bytes = io_timeout(HEADER_TIMEOUT, "l'en-tête média", secure.recv()).await?;
    let mut header: MediaStreamHeader = serde_json::from_slice(&header_bytes).map_err(to_io)?;
    let event_tx = ctx.event_tx.clone();

    // L'identité, la destination, le chemin et la taille sont validés avant de
    // solliciter l'utilisateur ou d'écrire quoi que ce soit. `requires_ack`
    // n'est qu'une donnée réseau non fiable : on le remplace par notre calcul.
    let requires_ack = validate_incoming_header(&header, &peer, &ctx.username)?;
    header.requires_ack = requires_ack;
    let id = header.media.id.clone();

    // Les médias de groupe passent par l'UI avant écriture : elle possède
    // l'état de membership nécessaire. Les petits transferts autorisés y sont
    // acceptés automatiquement.
    let needs_authorization = requires_ack
        || header
            .to_user
            .as_deref()
            .is_some_and(|to| to.starts_with('#'));
    if needs_authorization {
        let (decision_tx, decision_rx) = oneshot::channel::<bool>();
        let offer = MediaStreamOffer {
            from: header.from.clone(),
            to_user: header.to_user.clone(),
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

    let total = header.media.size_bytes;
    let path = media_dir.join(&id);
    if tokio::fs::try_exists(&path).await? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "identifiant de média déjà présent",
        ));
    }
    let temp_path = media_dir.join(format!(
        ".abcom-{}-{}.part",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));

    // Annonce le message à l'UI : la carte média apparaît avec sa progression.
    let _ = event_tx.send(AppEvent::MediaIncoming(header)).await;

    // En cas d'échec en cours de réception : on signale l'interruption (la carte
    // est retirée côté UI) et on supprime le fichier partiel.
    if let Err(e) = receive_body(
        &mut secure,
        &media_dir,
        &temp_path,
        &path,
        &id,
        total,
        &event_tx,
    )
    .await
    {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            let _ = event_tx.send(failed(&id, total, false)).await;
        }
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(e);
    }
    Ok(())
}

async fn receive_body(
    secure: &mut SecureStream,
    media_dir: &std::path::Path,
    temp_path: &std::path::Path,
    final_path: &std::path::Path,
    id: &str,
    total: u64,
    event_tx: &Sender<AppEvent>,
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(media_dir).await?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
        .await?;
    let mut received = 0u64;
    let mut reporter = ProgressReporter::new(false);
    while received < total {
        let chunk = io_timeout(BODY_CHUNK_TIMEOUT, "le corps média", secure.recv()).await?;
        if chunk.is_empty() {
            return Err(to_io("fin de flux média prématurée"));
        }
        validate_chunk_len(chunk.len(), total - received)?;
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        reporter.report(event_tx, id, received, total, false).await;
    }
    file.flush().await?;
    drop(file);
    rename_completed(temp_path, final_path).await?;
    reporter.report(event_tx, id, total, total, true).await;
    Ok(())
}

static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

async fn rename_completed(
    temp_path: &std::path::Path,
    final_path: &std::path::Path,
) -> std::io::Result<()> {
    // Le lien est créé atomiquement et échoue si l'identifiant existe déjà :
    // un pair ne peut pas remplacer un média mis en cache précédemment.
    tokio::fs::hard_link(temp_path, final_path).await?;
    tokio::fs::remove_file(temp_path).await
}

#[cfg(test)]
#[path = "../tests/test_network_media_stream.rs"]
mod tests;
