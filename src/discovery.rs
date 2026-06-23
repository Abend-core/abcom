use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;
use tokio::time::{interval, Duration};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::message::{AppEvent, DiscoveryPacket};

const BROADCAST_INTERVAL: u64 = 3;      // Envoyer un broadcast chaque 3 secondes
const DISCOVERY_TIMEOUT: u64 = 6;       // Un peer est inactif après 6 secondes d'inactivité (détection rapide changement réseau)
const CLEANUP_INTERVAL: u64 = 2;        // Vérifier les timeouts chaque 2 secondes

/// Crée le socket UDP de découverte avec SO_REUSEADDR/REUSEPORT, ce qui permet
/// à plusieurs instances locales de partager le même port de broadcast et donc
/// de se découvrir entre elles sur une seule machine.
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

    let packet = DiscoveryPacket { username: username.clone(), port: config::chat_port() };
    let data = serde_json::to_vec(&packet).unwrap_or_default();
    let broadcast_addr = format!("255.255.255.255:{}", config::DISCOVERY_PORT);

    let mut tick_broadcast = interval(Duration::from_secs(BROADCAST_INTERVAL));
    let mut tick_cleanup = interval(Duration::from_secs(CLEANUP_INTERVAL));
    let mut buf = vec![0u8; 1024];
    
    // Tracker les timestamps des peers découverts
    let mut peer_timestamps: HashMap<String, u64> = HashMap::new();

    loop {
        tokio::select! {
            _ = tick_broadcast.tick() => {
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
                            
                            peer_timestamps.insert(pkt.username.clone(), now);

                            // Adresse TCP du pair = IP source + port de chat annoncé
                            let tcp_addr = SocketAddr::new(addr.ip(), pkt.port);

                            // On envoie PeerDiscovered à chaque fois, l'UI gère les doublons
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
