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

pub struct ConnectionPool {
    ctx: Arc<NetContext>,
    conns: tokio::sync::Mutex<HashMap<SocketAddr, mpsc::Sender<NetworkPacket>>>,
}

impl ConnectionPool {
    pub fn new(ctx: Arc<NetContext>) -> Arc<Self> {
        Arc::new(Self {
            ctx,
            conns: tokio::sync::Mutex::new(HashMap::new()),
        })
    }

    /// Envoie un paquet au pair (connexion réutilisée ou établie à la volée).
    pub async fn send(self: &Arc<Self>, addr: SocketAddr, packet: NetworkPacket) {
        // Réutilisation de la connexion existante.
        let existing = self.conns.lock().await.get(&addr).cloned();
        if let Some(tx) = existing {
            match tx.send(packet).await {
                Ok(()) => return,
                Err(mpsc::error::SendError(returned)) => {
                    // Connexion morte : on la retire et on recompose.
                    self.conns.lock().await.remove(&addr);
                    return Box::pin(self.dial_and_send(addr, returned)).await;
                }
            }
        }
        self.dial_and_send(addr, packet).await;
    }

    async fn dial_and_send(self: &Arc<Self>, addr: SocketAddr, packet: NetworkPacket) {
        match self.connect(addr).await {
            Some(tx) => {
                let _ = tx.send(packet).await;
                self.conns.lock().await.insert(addr, tx);
            }
            None => {
                eprintln!("[network] Connexion sécurisée impossible vers {addr}");
            }
        }
    }

    /// Établit une connexion chiffrée : handshake Noise XX, échange des
    /// usernames, vérification TOFU, puis tâche d'écriture dédiée.
    async fn connect(&self, addr: SocketAddr) -> Option<mpsc::Sender<NetworkPacket>> {
        let mut stream = match TcpStream::connect(addr).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[network] Connexion échouée vers {addr}: {e}");
                return None;
            }
        };
        let (transport, remote_key) =
            match handshake_initiator(&mut stream, &self.ctx.identity, self.ctx.psk_bytes()).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[secure] Handshake échoué vers {addr}: {e}");
                    return None;
                }
            };
        let mut secure = SecureStream::new(stream, transport);
        let peer = match exchange_hello(&mut secure, &self.ctx.username, true).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[secure] Échange Hello échoué vers {addr}: {e}");
                return None;
            }
        };
        if self.ctx.trust.verify_and_pin(&peer, &remote_key) == Trust::Mismatch {
            self.ctx.report_key_mismatch(&peer).await;
            return None;
        }

        let (tx, mut rx) = mpsc::channel::<NetworkPacket>(64);
        tokio::spawn(async move {
            while let Some(packet) = rx.recv().await {
                let Ok(bytes) = serde_json::to_vec(&packet) else {
                    continue;
                };
                if secure.send(&bytes).await.is_err() {
                    break; // le pool recomposera à la prochaine émission
                }
            }
        });
        Some(tx)
    }
}
