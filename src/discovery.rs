use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;
use tokio::time::{interval, Duration};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

use crate::config;
use crate::identity::Identity;
use crate::message::{AppEvent, DiscoveryPacket};

const BROADCAST_INTERVAL: u64 = 3; // Envoyer un broadcast chaque 3 secondes
const DISCOVERY_TIMEOUT: u64 = 6; // Un peer est inactif après 6 secondes d'inactivité (détection rapide changement réseau)
const CLEANUP_INTERVAL: u64 = 2; // Vérifier les timeouts chaque 2 secondes
/// Écart maximal toléré entre l'horodatage d'une annonce et l'heure locale.
const MAX_ANNOUNCE_SKEW: u64 = 60;
/// Pairs distincts suivis simultanément.
///
/// Une annonce signée ne prouve que la possession d'une clé, pas une identité
/// distincte : une seule machine peut en fabriquer des milliers, chacune sous
/// un pseudo différent. Le plafond garde la découverte bornée.
const MAX_TRACKED_PEERS: usize = 512;

/// Groupe multicast de découverte (adresse administrativement scoupée).
const MULTICAST_GROUP: std::net::Ipv4Addr = std::net::Ipv4Addr::new(239, 255, 42, 98);

/// Crée le socket UDP de découverte. On combine SO_REUSEADDR/REUSEPORT et un
/// groupe multicast avec loopback activé : ainsi plusieurs instances sur une
/// même machine se découvrent (le broadcast `255.255.255.255` n'est pas rebouclé
/// localement sur macOS), tout en restant visibles sur le LAN. Le broadcast est
/// conservé en complément pour la compatibilité.
fn bind_discovery_socket() -> std::io::Result<UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};

    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;

    let addr: SocketAddr = format!("0.0.0.0:{}", config::DISCOVERY_PORT)
        .parse()
        .expect("adresse de découverte valide");
    socket.bind(&addr.into())?;

    // Découverte locale fiable via l'interface loopback (toujours routable, même
    // hors-ligne) : on rejoint le groupe multicast sur loopback, on y route le
    // trafic multicast sortant et on active le rebouclage local. La découverte
    // LAN, elle, reste assurée par le broadcast quand une route existe.
    let loopback = std::net::Ipv4Addr::LOCALHOST;
    socket.join_multicast_v4(&MULTICAST_GROUP, &loopback)?;
    let _ = socket.set_multicast_if_v4(&loopback);
    socket.set_multicast_loop_v4(true)?;

    UdpSocket::from_std(socket.into())
}

/// Tâche de découverte des pairs par UDP broadcast.
/// Diffuse le nom d'utilisateur toutes les 3 secondes et écoute les autres.
/// Détecte aussi les déconnexions quand un peer n'a pas répondu pendant 10s.
///
/// `peer_gone_tx` : chaque pair expiré, pour que le pool libère sa connexion.
pub async fn run(
    username: String,
    identity: Identity,
    tx: Sender<AppEvent>,
    peer_gone_tx: Sender<String>,
) {
    let socket = match bind_discovery_socket() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("erreur de bind : {}", e);
            return;
        }
    };

    let signing_key = identity.signing_key();
    let template = DiscoveryPacket {
        username: username.clone(),
        port: config::chat_port(),
        pubkey: identity.public_hex(),
        verifying_key: identity.verifying_hex(),
        sent_at: 0,
        signature: String::new(),
    };
    // On annonce à la fois en multicast (local + LAN, rebouclé) et en broadcast
    // (compatibilité avec d'anciens pairs).
    let multicast_addr = format!("{}:{}", MULTICAST_GROUP, config::DISCOVERY_PORT);
    let broadcast_addr = format!("255.255.255.255:{}", config::DISCOVERY_PORT);

    let mut tick_broadcast = interval(Duration::from_secs(BROADCAST_INTERVAL));
    let mut tick_cleanup = interval(Duration::from_secs(CLEANUP_INTERVAL));
    let mut buf = vec![0u8; 1024];

    // Tracker les timestamps et adresses des peers découverts (la fraîcheur
    // est gérée ici : l'UI n'est réveillée que sur changement d'état).
    let mut peer_timestamps: HashMap<String, u64> = HashMap::new();
    let mut peer_addrs: HashMap<String, SocketAddr> = HashMap::new();

    loop {
        tokio::select! {
            _ = tick_broadcast.tick() => {
                // Signée à chaque émission : l'horodatage borne la fenêtre de rejeu.
                let data = sign_announcement(&template, &signing_key, now_epoch());
                // Multicast (local fiable) + broadcast (LAN, best-effort).
                let _ = socket.send_to(&data, &multicast_addr).await;
                let _ = socket.send_to(&data, &broadcast_addr).await;
            }
            _ = tick_cleanup.tick() => {
                // Nettoyer les peers inactifs
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let disconnected: Vec<String> = peer_timestamps
                    .iter()
                    .filter(|(_, last_seen)| peer_is_stale(now, **last_seen))
                    .map(|(username, _)| username.clone())
                    .collect();

                for username in disconnected {
                    peer_timestamps.remove(&username);
                    peer_addrs.remove(&username);
                    let _ = peer_gone_tx.send(username.clone()).await;
                    let _ = tx.send(AppEvent::PeerDisconnected { username }).await;
                }
            }
            result = socket.recv_from(&mut buf) => {
                if let Ok((len, addr)) = result {
                    if let Ok(pkt) = serde_json::from_slice::<DiscoveryPacket>(&buf[..len]) {
                        // Ignorer son propre broadcast
                        if pkt.username != username
                            && crate::protocol::valid_username(&pkt.username)
                            && announcement_is_authentic(&pkt, now_epoch())
                        {
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();

                            // Adresse TCP du pair = IP source + port de chat annoncé
                            let tcp_addr = SocketAddr::new(addr.ip(), pkt.port);

                            // N'émettre PeerDiscovered que sur changement réel
                            // (nouveau pair, adresse changée, retour après
                            // déconnexion) : chaque événement réveille l'UI,
                            // les annonces périodiques ne doivent pas.
                            // Une seule machine peut signer autant d'annonces
                            // qu'elle veut, chacune sous un pseudo différent :
                            // sans plafond, les tables et la liste de l'UI
                            // enflent au rythme du réseau. Les pairs déjà connus
                            // continuent d'être rafraîchis.
                            let is_known = peer_timestamps.contains_key(&pkt.username);
                            if !is_known && peer_timestamps.len() >= MAX_TRACKED_PEERS {
                                tracing::warn!(
                                    "annonce ignorée : plafond de {MAX_TRACKED_PEERS} pairs atteint"
                                );
                                continue;
                            }
                            let is_new = peer_timestamps.insert(pkt.username.clone(), now).is_none();
                            if is_new {
                                crate::metrics::record_peer_seen();
                            }
                            let addr_changed = peer_addrs.insert(pkt.username.clone(), tcp_addr)
                                != Some(tcp_addr);
                            if is_new || addr_changed {
                                tracing::debug!("pair découvert : {} @ {tcp_addr}", pkt.username);
                                let _ = tx.send(AppEvent::PeerDiscovered {
                                    username: pkt.username,
                                    addr: tcp_addr,
                                }).await;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Un pair est-il silencieux depuis trop longtemps ?
///
/// `saturating_sub` et non `-` : une horloge corrigée en arrière (NTP, réglage
/// manuel, reprise de VM) rend `last_seen > now`. La soustraction déborderait
/// alors — tous les pairs seraient déclarés perdus d'un coup en release, et la
/// tâche de découverte paniquerait en debug, sans jamais redémarrer.
fn peer_is_stale(now: u64, last_seen: u64) -> bool {
    now.saturating_sub(last_seen) >= DISCOVERY_TIMEOUT
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Sérialise une annonce horodatée et signée.
fn sign_announcement(template: &DiscoveryPacket, key: &SigningKey, sent_at: u64) -> Vec<u8> {
    let mut packet = template.clone();
    packet.sent_at = sent_at;
    packet.signature = crate::identity::hex(&key.sign(&packet.signed_payload()).to_bytes());
    serde_json::to_vec(&packet).unwrap_or_default()
}

/// Vérifie qu'une annonce vient bien du détenteur de la clé annoncée et qu'elle est fraîche.
///
/// Ne protège pas la **première** rencontre TOFU : rien n'empêche un pair
/// d'annoncer le pseudo d'un autre avec sa propre clé, correctement signée.
/// Ce que ça ferme : annonces fabriquées pour une clé qu'on ne possède pas
/// (pairs fantômes) et rejeu d'annonces capturées.
fn announcement_is_authentic(packet: &DiscoveryPacket, now: u64) -> bool {
    if now.abs_diff(packet.sent_at) > MAX_ANNOUNCE_SKEW {
        return false;
    }
    let Some(verifying) = unhex(&packet.verifying_key)
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
    else {
        return false;
    };
    let Some(signature) = unhex(&packet.signature)
        .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
        .map(|bytes| Signature::from_bytes(&bytes))
    else {
        return false;
    };
    verifying
        .verify(&packet.signed_payload(), &signature)
        .is_ok()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
#[path = "tests/test_discovery.rs"]
mod tests;
