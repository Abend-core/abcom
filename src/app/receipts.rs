use std::net::SocketAddr;
use std::time::SystemTime;

use super::AppState;
use crate::message::ChatMessage;

/// Message en attente d'ACK
#[derive(Clone, Debug)]
pub struct PendingMessage {
    pub to_addr: SocketAddr,
    pub last_retry: SystemTime,
    pub retry_count: u32,
}

/// Liste nominative reçu/lu d'un message de groupe ou de « Tous »,
/// affichée par le popup « … » du fil (noms d'affichage, triés).
#[derive(Clone, Debug, Default)]
pub struct ReceiptDetail {
    pub delivered_by: Vec<String>,
    pub read_by: Vec<String>,
}

impl AppState {
    /// Calcule un hash FNV-1a stable entre processus pour identifier les messages.
    ///
    /// Contrairement à DefaultHasher (graine aléatoire depuis Rust 1.36),
    /// FNV-1a produit le même résultat quel que soit le processus ou la machine,
    /// ce qui est indispensable : Alice calcule le hash côté envoi, Bob le recalcule
    /// côté réception pour l'inclure dans l'ACK — ils doivent tomber sur la même valeur.
    ///
    /// La clé inclut `timestamp_epoch` + `to_user` + `media.id` pour éviter les collisions
    /// entre messages identiques ou vides (cas des médias), plus le `nonce`
    /// quand il existe : deux messages au contenu identique envoyés dans la
    /// même seconde restent distincts. Le nonce n'est ajouté que s'il est
    /// présent, pour ne pas changer le hash des messages déjà persistés.
    pub fn message_hash(msg: &ChatMessage) -> u64 {
        let key = format!(
            "{}:{}:{}:{}:{}{}",
            msg.from,
            msg.to_user.as_deref().unwrap_or("broadcast"),
            msg.timestamp_epoch.unwrap_or(0),
            msg.content,
            msg.media.as_ref().map(|m| m.id.as_str()).unwrap_or(""),
            msg.nonce.map(|n| format!(":{n}")).unwrap_or_default()
        );
        let mut hash: u64 = 14_695_981_039_346_656_037;
        for byte in key.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
        }
        hash
    }

    pub fn mark_message_read(&mut self, message_hash: u64, username: String) {
        self.read_receipts
            .entry(message_hash)
            .or_default()
            .insert(username);
        self.bump_content();
    }

    /// Enregistre un ACK de livraison nominatif reçu d'un pair (détail
    /// « reçu par » des salons et de « Tous »).
    pub fn mark_message_delivered_by(&mut self, message_hash: u64, username: String) {
        self.delivered_receipts
            .entry(message_hash)
            .or_default()
            .insert(username);
        self.bump_content();
    }

    /// Adresses des pairs qui doivent recevoir un ACK/ReadReceipt concernant `msg`.
    ///
    /// - message privé → seulement l'expéditeur ;
    /// - salon (`#…`) → tous les membres en ligne (moi exclu) ;
    /// - diffusion (« Tous », `to_user == None`) → tous les pairs en ligne.
    ///
    /// Diffuser à tout le salon permet à chaque membre d'accumuler le même
    /// état reçu/lu et donc d'afficher le détail (« … ») sur chaque message.
    pub fn receipt_recipients(&self, msg: &ChatMessage) -> Vec<SocketAddr> {
        match msg.to_user.as_deref() {
            Some(target) if !target.starts_with('#') => self
                .peers
                .iter()
                .find(|p| p.username == msg.from && p.online && !p.addr.ip().is_unspecified())
                .map(|p| p.addr)
                .into_iter()
                .collect(),
            Some(group_key) => group_key
                .strip_prefix('#')
                .map(|g| self.group_member_addrs(g))
                .unwrap_or_default(),
            None => self
                .peers
                .iter()
                .filter(|p| {
                    p.online && p.username != self.my_username && !p.addr.ip().is_unspecified()
                })
                .map(|p| p.addr)
                .collect(),
        }
    }

    /// Liste nominative reçu/lu d'un message (noms d'affichage, triés),
    /// pour le popup « … » des salons et de « Tous ».
    pub fn receipt_detail(&self, message_hash: u64) -> ReceiptDetail {
        let resolve = |users: Option<&std::collections::HashSet<String>>| {
            let mut names: Vec<String> = users
                .map(|set| set.iter().map(|u| self.peer_display_name(u)).collect())
                .unwrap_or_default();
            names.sort();
            names
        };
        ReceiptDetail {
            delivered_by: resolve(self.delivered_receipts.get(&message_hash)),
            read_by: resolve(self.read_receipts.get(&message_hash)),
        }
    }

    #[allow(dead_code)]
    pub fn is_message_read_by(&self, message_hash: u64, username: &str) -> bool {
        self.read_receipts
            .get(&message_hash)
            .map(|r| r.contains(username))
            .unwrap_or(false)
    }

    pub fn get_read_count(&self, message_hash: u64) -> usize {
        self.read_receipts
            .get(&message_hash)
            .map(|r| r.len())
            .unwrap_or(0)
    }

    /// Marque un message comme envoyé (en attente d'ACK)
    pub fn mark_message_sent(&mut self, message_hash: u64, to_addr: SocketAddr) {
        self.pending_messages.insert(
            message_hash,
            PendingMessage {
                to_addr,
                last_retry: SystemTime::now(),
                retry_count: 0,
            },
        );
        self.bump_content();
    }

    pub fn mark_message_acked(&mut self, message_hash: u64) {
        if self.pending_messages.remove(&message_hash).is_some() {
            self.bump_content();
        }
    }

    /// Retourne les messages qui doivent être retransmis (backoff exponentiel)
    pub fn get_retry_messages(&mut self) -> Vec<(u64, SocketAddr)> {
        let now = SystemTime::now();
        let mut to_retry = Vec::new();
        for (hash, pending) in &self.pending_messages {
            let delay = 2u64.saturating_pow(pending.retry_count.min(5));
            if let Ok(elapsed) = now.duration_since(pending.last_retry) {
                if elapsed.as_secs() >= delay {
                    to_retry.push((*hash, pending.to_addr));
                }
            }
        }
        for (hash, _) in &to_retry {
            if let Some(p) = self.pending_messages.get_mut(hash) {
                p.retry_count += 1;
                p.last_retry = now;
            }
        }
        to_retry
    }

    #[allow(dead_code)]
    pub fn is_message_pending(&self, message_hash: u64) -> bool {
        self.pending_messages.contains_key(&message_hash)
    }
}

#[cfg(test)]
#[path = "../tests/test_app_receipts.rs"]
mod tests;
