use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;
use tokio::time::{interval, Duration};

use crate::config;
use crate::message::{AppEvent, DiscoveryPacket};

const BROADCAST_INTERVAL: u64 = 3; // Envoyer un broadcast chaque 3 secondes
const DISCOVERY_TIMEOUT: u64 = 6; // Un peer est inactif après 6 secondes d'inactivité (détection rapide changement réseau)
const CLEANUP_INTERVAL: u64 = 2; // Vérifier les timeouts chaque 2 secondes

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
pub async fn run(username: String, tx: Sender<AppEvent>) {
    let socket = match bind_discovery_socket() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[discovery] Erreur de bind: {}", e);
            return;
        }
    };

    let packet = DiscoveryPacket {
        username: username.clone(),
        port: config::chat_port(),
    };
    let data = serde_json::to_vec(&packet).unwrap_or_default();
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
                    .filter(|(_, last_seen)| now - *last_seen >= DISCOVERY_TIMEOUT)
                    .map(|(username, _)| username.clone())
                    .collect();

                for username in disconnected {
                    peer_timestamps.remove(&username);
                    peer_addrs.remove(&username);
                    let _ = tx.send(AppEvent::PeerDisconnected { username }).await;
                }
            }
            result = socket.recv_from(&mut buf) => {
                if let Ok((len, addr)) = result {
                    if let Ok(pkt) = serde_json::from_slice::<DiscoveryPacket>(&buf[..len]) {
                        // Ignorer son propre broadcast
                        if pkt.username != username {
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
                            let is_new = peer_timestamps.insert(pkt.username.clone(), now).is_none();
                            let addr_changed = peer_addrs.insert(pkt.username.clone(), tcp_addr)
                                != Some(tcp_addr);
                            if is_new || addr_changed {
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

#[cfg(test)]
#[path = "tests/test_discovery.rs"]
mod tests;
