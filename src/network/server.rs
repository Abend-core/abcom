//! Serveur de chat : connexions TCP entrantes **persistantes et chiffrées**
//! (Noise XX). Chaque connexion fait un handshake, échange les usernames
//! (Hello), vérifie la clé du pair (TOFU) puis dispatche les paquets reçus
//! en boucle jusqu'à la fermeture.

use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};

use crate::config;
use crate::message::{AppEvent, NetworkPacket};

use super::secure::{exchange_hello, handshake_responder, SecureStream, Trust};
use super::NetContext;
use crate::util::MutexExt;

const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CONNECTION_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6 * 60);
const MAX_INCOMING_CONNECTIONS: usize = 64;

/// Serveur TCP : écoute les connexions entrantes et dispatche les événements.
pub async fn run_server(ctx: Arc<NetContext>) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", config::chat_port())).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("erreur de bind TCP : {}", e);
            return;
        }
    };
    run_server_on(listener, ctx).await;
}

/// Boucle d'acceptation sur un listener déjà lié (testable sur port éphémère).
pub async fn run_server_on(listener: TcpListener, ctx: Arc<NetContext>) {
    let connection_limit = Arc::new(tokio::sync::Semaphore::new(MAX_INCOMING_CONNECTIONS));
    let active_peers = Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                // Cf. pool.rs : les accusés et indicateurs de frappe repartent
                // par cette socket, Nagle y ajouterait un délai perceptible.
                let _ = stream.set_nodelay(true);
                let Ok(permit) = connection_limit.clone().try_acquire_owned() else {
                    tracing::warn!("connexion entrante refusée : limite atteinte");
                    continue;
                };
                let ctx = ctx.clone();
                let active_peers = active_peers.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(e) = handle_incoming_tracked(stream, ctx, Some(active_peers)).await {
                        tracing::debug!("connexion entrante terminée : {e}");
                    }
                });
            }
            Err(_) => continue,
        }
    }
}

/// Gère une connexion entrante : handshake, identification, puis boucle de
/// réception des paquets (la connexion reste ouverte).
#[cfg(test)]
async fn handle_incoming(stream: TcpStream, ctx: Arc<NetContext>) -> std::io::Result<()> {
    handle_incoming_tracked(stream, ctx, None).await
}

struct ActivePeerGuard {
    peer: String,
    active: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

impl Drop for ActivePeerGuard {
    fn drop(&mut self) {
        self.active.lock_safe().remove(&self.peer);
    }
}

#[tracing::instrument(skip_all)]
async fn handle_incoming_tracked(
    mut stream: TcpStream,
    ctx: Arc<NetContext>,
    active: Option<Arc<std::sync::Mutex<std::collections::HashSet<String>>>>,
) -> std::io::Result<()> {
    let (transport, remote_key) = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        handshake_responder(&mut stream, &ctx.identity, ctx.psk_bytes()),
    )
    .await
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "handshake expiré"))??;
    let mut secure = SecureStream::new(stream, transport);
    let peer = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        exchange_hello(&mut secure, &ctx.username, false),
    )
    .await
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Hello expiré"))??;
    if ctx.trust.verify_and_pin(&peer, &remote_key) == Trust::Mismatch {
        ctx.report_key_mismatch(&peer).await;
        return Ok(());
    }
    let _active_guard = if let Some(active) = active {
        if !active.lock_safe().insert(peer.clone()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "une session entrante existe déjà pour ce pair",
            ));
        }
        Some(ActivePeerGuard {
            peer: peer.clone(),
            active,
        })
    } else {
        None
    };

    loop {
        let bytes = match tokio::time::timeout(CONNECTION_IDLE_TIMEOUT, secure.recv()).await {
            Ok(Ok(bytes)) => bytes,
            Ok(Err(_)) | Err(_) => return Ok(()),
        };
        dispatch_packet(&bytes, &peer, &ctx.username, &ctx.event_tx).await;
    }
}

/// Dispatche un paquet JSON déchiffré vers les événements de l'UI.
async fn dispatch_packet(
    bytes: &[u8],
    peer: &str,
    local_username: &str,
    tx: &tokio::sync::mpsc::Sender<AppEvent>,
) {
    let packet = match serde_json::from_slice::<NetworkPacket>(bytes) {
        Ok(packet) => packet,
        Err(_) => {
            tracing::warn!("paquet entrant non reconnu ({} bytes)", bytes.len());
            return;
        }
    };
    if !packet_matches_peer(&packet, peer, local_username) {
        crate::metrics::record_packet_dropped();
        tracing::warn!("paquet entrant rejeté pour le pair authentifié {peer}");
        return;
    }
    crate::metrics::record_packet_received();

    match packet {
        NetworkPacket::Chat(msg) => {
            let _ = tx.send(AppEvent::MessageReceived(msg)).await;
        }
        NetworkPacket::Group(event) => {
            let _ = tx
                .send(AppEvent::GroupEventReceived {
                    peer: peer.to_string(),
                    event,
                })
                .await;
        }
        NetworkPacket::Typing(indicator) => {
            let _ = tx.send(AppEvent::UserTyping(indicator.from)).await;
        }
        NetworkPacket::ReadReceipt(r) => {
            let _ = tx.send(AppEvent::ReadReceiptReceived(r)).await;
        }
        NetworkPacket::Ack(ack) => {
            let _ = tx.send(AppEvent::MessageAckReceived(ack)).await;
        }
        NetworkPacket::Avatar(announce) => {
            let _ = tx.send(AppEvent::AvatarReceived(announce)).await;
        }
        NetworkPacket::Reaction(event) => {
            let _ = tx.send(AppEvent::ReactionReceived(event)).await;
        }
    }
}

fn packet_matches_peer(packet: &NetworkPacket, peer: &str, local_username: &str) -> bool {
    match packet {
        NetworkPacket::Chat(msg) => {
            msg.from == peer
                && msg
                    .to_user
                    .as_deref()
                    .is_none_or(|to| to == local_username || to.starts_with('#'))
        }
        NetworkPacket::Group(_) => true,
        NetworkPacket::Typing(indicator) => indicator.from == peer,
        NetworkPacket::ReadReceipt(receipt) => receipt.from == peer && receipt.to == local_username,
        NetworkPacket::Ack(ack) => ack.from == peer && ack.to == local_username,
        NetworkPacket::Avatar(announce) => announce.from == peer,
        NetworkPacket::Reaction(event) => event.user == peer,
    }
}

#[cfg(test)]
#[path = "../tests/test_network_server.rs"]
mod tests;
