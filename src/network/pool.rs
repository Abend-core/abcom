//! Connexions TCP persistantes sortantes, chiffrées Noise, une par pair.
//!
//! Remplace le modèle « une connexion par paquet » : la première émission
//! vers une adresse ouvre la connexion (handshake + Hello + TOFU), les
//! suivantes réutilisent le canal. Une erreur d'écriture ferme la connexion,
//! qui sera recomposée à la prochaine émission.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::message::NetworkPacket;

use super::secure::{exchange_hello, handshake_initiator, SecureStream, Trust};
use super::NetContext;

const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CONNECTION_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5 * 60);

pub struct ConnectionPool {
    ctx: Arc<NetContext>,
    conns: tokio::sync::Mutex<HashMap<(String, SocketAddr), mpsc::Sender<NetworkPacket>>>,
}

impl ConnectionPool {
    pub fn new(ctx: Arc<NetContext>) -> Arc<Self> {
        Arc::new(Self {
            ctx,
            conns: tokio::sync::Mutex::new(HashMap::new()),
        })
    }

    /// Envoie un paquet au pair (connexion réutilisée ou établie à la volée).
    pub async fn send(
        self: &Arc<Self>,
        expected_peer: &str,
        addr: SocketAddr,
        packet: NetworkPacket,
    ) {
        let key = (expected_peer.to_string(), addr);
        // Réutilisation de la connexion existante.
        let existing = {
            let mut conns = self.conns.lock().await;
            conns.retain(|_, sender| !sender.is_closed());
            conns.get(&key).cloned()
        };
        if let Some(tx) = existing {
            match tx.send(packet).await {
                Ok(()) => return,
                Err(mpsc::error::SendError(returned)) => {
                    // Connexion morte : on la retire et on recompose.
                    self.conns.lock().await.remove(&key);
                    return Box::pin(self.dial_and_send(expected_peer, addr, returned)).await;
                }
            }
        }
        self.dial_and_send(expected_peer, addr, packet).await;
    }

    async fn dial_and_send(
        self: &Arc<Self>,
        expected_peer: &str,
        addr: SocketAddr,
        packet: NetworkPacket,
    ) {
        match self.connect(expected_peer, addr).await {
            Some(tx) => {
                let _ = tx.send(packet).await;
                self.conns
                    .lock()
                    .await
                    .insert((expected_peer.to_string(), addr), tx);
            }
            None => {
                tracing::warn!("connexion sécurisée impossible vers {addr}");
            }
        }
    }

    /// Établit une connexion chiffrée : handshake Noise XX, échange des
    /// usernames, vérification TOFU, puis tâche d'écriture dédiée.
    async fn connect(
        &self,
        expected_peer: &str,
        addr: SocketAddr,
    ) -> Option<mpsc::Sender<NetworkPacket>> {
        let mut stream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                tracing::warn!("connexion échouée vers {addr}: {e}");
                return None;
            }
            Err(_) => {
                tracing::warn!("connexion expirée vers {addr}");
                return None;
            }
        };
        let handshake = handshake_initiator(&mut stream, &self.ctx.identity, self.ctx.psk_bytes());
        let (transport, remote_key) = match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake).await
        {
            Ok(Ok(value)) => value,
            Ok(Err(e)) => {
                tracing::warn!("handshake échoué vers {addr}: {e}");
                return None;
            }
            Err(_) => {
                tracing::warn!("handshake expiré vers {addr}");
                return None;
            }
        };
        let mut secure = SecureStream::new(stream, transport);
        let hello = exchange_hello(&mut secure, &self.ctx.username, true);
        let peer = match tokio::time::timeout(HANDSHAKE_TIMEOUT, hello).await {
            Ok(Ok(peer)) => peer,
            Ok(Err(e)) => {
                tracing::warn!("échange Hello échoué vers {addr}: {e}");
                return None;
            }
            Err(_) => {
                tracing::warn!("échange Hello expiré vers {addr}");
                return None;
            }
        };
        if peer != expected_peer {
            tracing::warn!("identité inattendue vers {addr}: {peer} au lieu de {expected_peer}");
            return None;
        }
        if self.ctx.trust.verify_and_pin(&peer, &remote_key) == Trust::Mismatch {
            self.ctx.report_key_mismatch(&peer).await;
            return None;
        }

        let (tx, mut rx) = mpsc::channel::<NetworkPacket>(64);
        tokio::spawn(async move {
            loop {
                let packet = match tokio::time::timeout(CONNECTION_IDLE_TIMEOUT, rx.recv()).await {
                    Ok(Some(packet)) => packet,
                    Ok(None) => break,
                    Err(_) => {
                        rx.close();
                        while let Some(packet) = rx.recv().await {
                            let Ok(bytes) = serde_json::to_vec(&packet) else {
                                continue;
                            };
                            if !matches!(
                                tokio::time::timeout(WRITE_TIMEOUT, secure.send(&bytes)).await,
                                Ok(Ok(()))
                            ) {
                                break;
                            }
                        }
                        break;
                    }
                };
                let Ok(bytes) = serde_json::to_vec(&packet) else {
                    continue;
                };
                if !matches!(
                    tokio::time::timeout(WRITE_TIMEOUT, secure.send(&bytes)).await,
                    Ok(Ok(()))
                ) {
                    break; // le pool recomposera à la prochaine émission
                }
            }
        });
        Some(tx)
    }
}
