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

/// Serveur TCP : écoute les connexions entrantes et dispatche les événements.
pub async fn run_server(ctx: Arc<NetContext>) {
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", config::chat_port())).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[network] Erreur de bind TCP: {}", e);
            return;
        }
    };
    run_server_on(listener, ctx).await;
}

/// Boucle d'acceptation sur un listener déjà lié (testable sur port éphémère).
pub async fn run_server_on(listener: TcpListener, ctx: Arc<NetContext>) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_incoming(stream, ctx).await {
                        eprintln!("[network] Connexion entrante terminée : {e}");
                    }
                });
            }
            Err(_) => continue,
        }
    }
}

/// Gère une connexion entrante : handshake, identification, puis boucle de
/// réception des paquets (la connexion reste ouverte).
async fn handle_incoming(mut stream: TcpStream, ctx: Arc<NetContext>) -> std::io::Result<()> {
    let (transport, remote_key) =
        handshake_responder(&mut stream, &ctx.identity, ctx.psk_bytes()).await?;
    let mut secure = SecureStream::new(stream, transport);
    let peer = exchange_hello(&mut secure, &ctx.username, false).await?;
    if ctx.trust.verify_and_pin(&peer, &remote_key) == Trust::Mismatch {
        ctx.report_key_mismatch(&peer).await;
        return Ok(());
    }

    loop {
        let bytes = match secure.recv().await {
            Ok(b) => b,
            Err(_) => return Ok(()), // fermeture (normale ou non) du pair
        };
        dispatch_packet(&bytes, &ctx.event_tx).await;
    }
}

/// Dispatche un paquet JSON déchiffré vers les événements de l'UI.
async fn dispatch_packet(bytes: &[u8], tx: &tokio::sync::mpsc::Sender<AppEvent>) {
    match serde_json::from_slice::<NetworkPacket>(bytes) {
        Ok(NetworkPacket::Chat(msg)) => {
            let _ = tx.send(AppEvent::MessageReceived(msg)).await;
        }
        Ok(NetworkPacket::Group(event)) => {
            let _ = tx.send(AppEvent::GroupEventReceived(event)).await;
        }
        Ok(NetworkPacket::Typing(indicator)) => {
            let _ = tx.send(AppEvent::UserTyping(indicator.from)).await;
        }
        Ok(NetworkPacket::ReadReceipt(r)) => {
            let _ = tx.send(AppEvent::ReadReceiptReceived(r)).await;
        }
        Ok(NetworkPacket::Ack(ack)) => {
            let _ = tx.send(AppEvent::MessageAckReceived(ack)).await;
        }
        Ok(NetworkPacket::Avatar(announce)) => {
            let _ = tx.send(AppEvent::AvatarReceived(announce)).await;
        }
        Ok(NetworkPacket::Reaction(event)) => {
            let _ = tx.send(AppEvent::ReactionReceived(event)).await;
        }
        Err(_) => eprintln!(
            "[network] Paquet entrant non reconnu ({} bytes)",
            bytes.len()
        ),
    }
}

#[cfg(test)]
#[path = "../tests/test_network_server.rs"]
mod tests;
