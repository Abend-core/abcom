//! Compteurs de la session en cours (Paramètres → Diagnostic) : diagnostiquer sans logging verbeux.
//!
//! Non persistés et `Relaxed` : rien ne dépend de leur cohérence instantanée.

use std::sync::atomic::{AtomicU64, Ordering};

static PACKETS_SENT: AtomicU64 = AtomicU64::new(0);
static PACKETS_RECEIVED: AtomicU64 = AtomicU64::new(0);
static PACKETS_DROPPED: AtomicU64 = AtomicU64::new(0);
static PEERS_SEEN: AtomicU64 = AtomicU64::new(0);

/// Photographie des compteurs à un instant donné (affichage Paramètres).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Paquets remis à une connexion chiffrée (chat, ACK, réactions…).
    pub packets_sent: u64,
    /// Paquets entrants acceptés après vérification d'identité.
    pub packets_received: u64,
    /// Paquets jetés : file saturée, connexion impossible, taille excessive.
    pub packets_dropped: u64,
    /// Pairs distincts vus par la découverte depuis le lancement.
    pub peers_seen: u64,
}

pub fn record_packet_sent() {
    PACKETS_SENT.fetch_add(1, Ordering::Relaxed);
}

pub fn record_packet_received() {
    PACKETS_RECEIVED.fetch_add(1, Ordering::Relaxed);
}

/// Paquet perdu (file pleine, pair injoignable, taille excessive) — rend visibles les pertes `try_send`.
pub fn record_packet_dropped() {
    PACKETS_DROPPED.fetch_add(1, Ordering::Relaxed);
}

pub fn record_peer_seen() {
    PEERS_SEEN.fetch_add(1, Ordering::Relaxed);
}

/// Répartition de la mémoire du processus (Paramètres → Diagnostic).
///
/// L'écart entre `heap` et `rss` est ce qui ne passe pas par notre allocateur :
/// pilote graphique, images GPU, DLL, piles de threads. Le mesurer évite de
/// chercher une fuite dans le code Rust quand le gros du poids est ailleurs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    /// Octets réellement engagés par mimalloc — le tas Rust.
    pub heap: u64,
    /// Pic du tas depuis le lancement.
    pub heap_peak: u64,
    /// Pages touchées par le processus entier (working set).
    pub rss: u64,
    /// Pic du working set.
    pub rss_peak: u64,
}

/// Relève les compteurs de l'allocateur. Sur Windows, `rss` et `commit` sont
/// exacts ; ailleurs, `rss` est estimé à partir du commit.
pub fn memory() -> Memory {
    let (mut rss, mut rss_peak, mut heap, mut heap_peak) = (0usize, 0usize, 0usize, 0usize);
    // Sûr : huit pointeurs de sortie optionnels, ceux qui ne nous intéressent
    // pas sont nuls.
    unsafe {
        libmimalloc_sys::mi_process_info(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut rss,
            &mut rss_peak,
            &mut heap,
            &mut heap_peak,
            std::ptr::null_mut(),
        );
    }
    Memory {
        heap: heap as u64,
        heap_peak: heap_peak as u64,
        rss: rss as u64,
        rss_peak: rss_peak as u64,
    }
}

pub fn snapshot() -> Snapshot {
    Snapshot {
        packets_sent: PACKETS_SENT.load(Ordering::Relaxed),
        packets_received: PACKETS_RECEIVED.load(Ordering::Relaxed),
        packets_dropped: PACKETS_DROPPED.load(Ordering::Relaxed),
        peers_seen: PEERS_SEEN.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_are_monotonic() {
        let before = snapshot();
        record_packet_sent();
        record_packet_received();
        record_packet_dropped();
        record_peer_seen();
        let after = snapshot();
        assert!(after.packets_sent > before.packets_sent);
        assert!(after.packets_received > before.packets_received);
        assert!(after.packets_dropped > before.packets_dropped);
        assert!(after.peers_seen > before.peers_seen);
    }
}
