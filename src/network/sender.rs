//! Expéditeurs : les canaux typés de l'UI convergent vers le
//! [`ConnectionPool`](super::pool::ConnectionPool), qui maintient **une
//! connexion persistante et chiffrée par pair** (plus de connexion TCP par
//! paquet).

use std::sync::Arc;
use std::{collections::HashMap, net::SocketAddr};

use tokio::sync::mpsc::{self, Receiver};

use crate::message::{NetworkPacket, NetworkSendRequest};

use super::pool::ConnectionPool;

const WORKER_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// Attente maximale d'une place dans la file d'un pair pour un paquet critique.
const QUEUE_BACKPRESSURE: std::time::Duration = std::time::Duration::from_secs(5);

/// Expéditeur commun pour tous les paquets courts.
pub async fn run_sender(mut rx: Receiver<NetworkSendRequest>, pool: Arc<ConnectionPool>) {
    let mut workers: HashMap<(String, SocketAddr), mpsc::Sender<NetworkPacket>> = HashMap::new();
    while let Some(req) = rx.recv().await {
        workers.retain(|_, worker| !worker.is_closed());
        let key = (req.to_peer, req.to_addr);
        let worker_key = key.clone();
        let worker = workers.entry(key.clone()).or_insert_with(|| {
            let (tx, mut peer_rx) = mpsc::channel(64);
            let pool = pool.clone();
            tokio::spawn(async move {
                loop {
                    let packet =
                        match tokio::time::timeout(WORKER_IDLE_TIMEOUT, peer_rx.recv()).await {
                            Ok(Some(packet)) => packet,
                            Ok(None) => break,
                            Err(_) => {
                                peer_rx.close();
                                while let Some(packet) = peer_rx.recv().await {
                                    pool.send(&worker_key.0, worker_key.1, packet).await;
                                }
                                break;
                            }
                        };
                    pool.send(&worker_key.0, worker_key.1, packet).await;
                }
            });
            tx
        });
        // File du pair pleine : on ne jette que ce dont la perte est sans
        // conséquence. Le reste applique une contre-pression bornée — bornée,
        // pour qu'un pair bloqué ne fige pas la distribution vers les autres.
        match worker.try_send(req.packet) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Closed(_)) => {
                crate::metrics::record_packet_dropped();
                tracing::warn!("file réseau fermée : {}", key.0);
            }
            Err(mpsc::error::TrySendError::Full(packet)) => {
                if packet.is_droppable() {
                    crate::metrics::record_packet_dropped();
                    tracing::debug!("signal abandonné, file du pair pleine : {}", key.0);
                } else {
                    match tokio::time::timeout(QUEUE_BACKPRESSURE, worker.send(packet)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(_)) | Err(_) => {
                            crate::metrics::record_packet_dropped();
                            tracing::warn!("paquet perdu, file du pair saturée : {}", key.0);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/test_network_sender.rs"]
mod tests;
