use tokio::net::UdpSocket;
use tokio::sync::mpsc::Sender;
use tokio::time::{interval, Duration};

use crate::message::{AppEvent, DiscoveryPacket};

pub const DISCOVERY_PORT: u16 = 9001;

/// Tâche de découverte des pairs par UDP broadcast.
/// Diffuse le nom d'utilisateur toutes les 3 secondes et écoute les autres.
pub async fn run(username: String, tx: Sender<AppEvent>) {
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", DISCOVERY_PORT)).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[discovery] Erreur de bind: {}", e);
            return;
        }
    };

    if let Err(e) = socket.set_broadcast(true) {
        eprintln!("[discovery] Impossible d'activer le broadcast: {}", e);
    }

    let packet = DiscoveryPacket { username: username.clone() };
    let data = serde_json::to_vec(&packet).unwrap_or_default();
    let broadcast_addr = format!("255.255.255.255:{}", DISCOVERY_PORT);

    let mut tick = interval(Duration::from_secs(3));
    let mut buf = vec![0u8; 1024];

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let _ = socket.send_to(&data, &broadcast_addr).await;
            }
            result = socket.recv_from(&mut buf) => {
                if let Ok((len, addr)) = result {
                    if let Ok(pkt) = serde_json::from_slice::<DiscoveryPacket>(&buf[..len]) {
                        // Ignorer son propre broadcast
                        if pkt.username != username {
                            let _ = tx.send(AppEvent::PeerDiscovered {
                                username: pkt.username,
                                addr,
                            }).await;
                        }
                    }
                }
            }
        }
    }
}
