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
use crate::metrics;
use crate::util::MutexExt;

use super::secure::{
    exchange_hello, handshake_initiator, SecureStream, Trust, MAX_LOGICAL_MESSAGE,
};
use super::NetContext;

const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CONNECTION_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// Période de balayage des connexions fermées (tâches d'écriture terminées).
const SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// Intervalle minimal entre deux alertes d'échec d'envoi pour un même pair :
/// un pair injoignable échouerait sinon à chaque paquet (retry, frappe, ACK).
const FAILURE_REPORT_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(30);

/// (pair attendu, adresse) : le même pair à une nouvelle adresse est une connexion distincte.
type ConnKey = (String, SocketAddr);
/// Extrémité d'écriture : des paquets déjà sérialisés par [`ConnectionPool::send`].
type ConnSender = mpsc::Sender<Vec<u8>>;

pub struct ConnectionPool {
    ctx: Arc<NetContext>,
    conns: tokio::sync::Mutex<HashMap<ConnKey, ConnSender>>,
    /// Dernière alerte remontée à l'UI par pair (anti-spam de la bannière).
    failure_reports: std::sync::Mutex<HashMap<String, std::time::Instant>>,
}

impl ConnectionPool {
    pub fn new(ctx: Arc<NetContext>) -> Arc<Self> {
        Arc::new(Self {
            ctx,
            conns: tokio::sync::Mutex::new(HashMap::new()),
            failure_reports: std::sync::Mutex::new(HashMap::new()),
        })
    }

    /// Purge les connexions closes : sinon la map ne se nettoie qu'à la prochaine émission.
    pub async fn sweep_closed(self: Arc<Self>) {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            let mut conns = self.conns.lock().await;
            let before = conns.len();
            conns.retain(|_, sender| !sender.is_closed());
            if conns.len() != before {
                tracing::debug!(
                    "pool : {} connexion(s) fermée(s) purgée(s)",
                    before - conns.len()
                );
            }
        }
    }

    /// Ferme et oublie les connexions vers un pair qui vient de disparaître
    /// de la découverte (son adresse ne sera plus valide).
    pub async fn drop_peer(&self, username: &str) {
        self.conns
            .lock()
            .await
            .retain(|(peer, _), _| peer != username);
    }

    /// Envoie un paquet ; sérialisation et garde-fou de taille en un point unique, avant que
    /// le récepteur ne coupe la connexion pour dépassement de `MAX_LOGICAL_MESSAGE`.
    pub async fn send(
        self: &Arc<Self>,
        expected_peer: &str,
        addr: SocketAddr,
        packet: NetworkPacket,
    ) {
        let bytes = match serde_json::to_vec(&packet) {
            Ok(bytes) => bytes,
            Err(error) => {
                metrics::record_packet_dropped();
                tracing::error!("paquet non sérialisable pour {expected_peer} : {error}");
                return;
            }
        };
        if bytes.len() > MAX_LOGICAL_MESSAGE {
            metrics::record_packet_dropped();
            tracing::warn!(
                "paquet de {} octets refusé pour {expected_peer} (limite {MAX_LOGICAL_MESSAGE})",
                bytes.len()
            );
            return;
        }

        let key = (expected_peer.to_string(), addr);
        // Réutilisation de la connexion existante.
        let existing = {
            let mut conns = self.conns.lock().await;
            conns.retain(|_, sender| !sender.is_closed());
            conns.get(&key).cloned()
        };
        if let Some(tx) = existing {
            match tx.send(bytes).await {
                Ok(()) => {
                    metrics::record_packet_sent();
                    return;
                }
                Err(mpsc::error::SendError(returned)) => {
                    // Connexion morte : on la retire et on recompose.
                    self.conns.lock().await.remove(&key);
                    return Box::pin(self.dial_and_send(expected_peer, addr, returned)).await;
                }
            }
        }
        self.dial_and_send(expected_peer, addr, bytes).await;
    }

    async fn dial_and_send(
        self: &Arc<Self>,
        expected_peer: &str,
        addr: SocketAddr,
        bytes: Vec<u8>,
    ) {
        match self.connect(expected_peer, addr).await {
            Some(tx) => {
                if tx.send(bytes).await.is_ok() {
                    metrics::record_packet_sent();
                }
                self.conns
                    .lock()
                    .await
                    .insert((expected_peer.to_string(), addr), tx);
            }
            None => {
                metrics::record_packet_dropped();
                self.report_send_failure(expected_peer).await;
            }
        }
    }

    /// Remonte l'échec à la bannière : sur un binaire release sans console, les logs ne se voient pas.
    async fn report_send_failure(&self, peer: &str) {
        let now = std::time::Instant::now();
        let should_report = {
            let mut reports = self.failure_reports.lock_safe();
            reports.retain(|_, at| now.duration_since(*at) < FAILURE_REPORT_COOLDOWN);
            match reports.get(peer) {
                Some(_) => false,
                None => {
                    reports.insert(peer.to_string(), now);
                    true
                }
            }
        };
        tracing::warn!("connexion sécurisée impossible vers « {peer} »");
        if should_report {
            let _ = self
                .ctx
                .event_tx
                .send(crate::message::AppEvent::SendFailed {
                    username: peer.to_string(),
                })
                .await;
        }
    }

    /// Établit une connexion chiffrée : handshake Noise XX, échange des
    /// usernames, vérification TOFU, puis tâche d'écriture dédiée.
    #[tracing::instrument(skip(self), fields(peer = expected_peer, %addr))]
    async fn connect(&self, expected_peer: &str, addr: SocketAddr) -> Option<ConnSender> {
        let mut stream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await
        {
            Ok(Ok(stream)) => {
                // Notre trafic est fait de petits paquets : Nagle les retiendrait
                // jusqu'à 40 ms chacun, sur un LAN à moins d'une milliseconde.
                let _ = stream.set_nodelay(true);
                stream
            }
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

        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
        tokio::spawn(async move {
            loop {
                let bytes = match tokio::time::timeout(CONNECTION_IDLE_TIMEOUT, rx.recv()).await {
                    Ok(Some(bytes)) => bytes,
                    Ok(None) => break,
                    Err(_) => {
                        // Inactivité : vider la file avant de fermer, sinon on perd des paquets acceptés.
                        rx.close();
                        while let Some(bytes) = rx.recv().await {
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
