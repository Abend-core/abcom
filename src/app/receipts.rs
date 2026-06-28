use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::AppState;
use crate::message::ChatMessage;

/// Message en attente d'ACK (session uniquement, non persisté)
#[derive(Clone, Debug)]
pub struct PendingMessage {
    pub to_addr: SocketAddr,
    pub last_retry: SystemTime,
    pub retry_count: u32,
}

/// État de livraison/lecture persisté pour un message identifié par son hash.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ReceiptEntry {
    /// Utilisateurs ayant envoyé un ACK de livraison.
    pub delivered_by: HashSet<String>,
    /// Utilisateurs ayant envoyé un ReadReceipt (lecture).
    pub read_by: HashSet<String>,
}

/// Vue calculée à la volée pour l'UI : état synthétique + détail pour le popup groupe.
#[derive(Clone, Debug)]
pub struct ReceiptState {
    /// Toujours en transit (dans pending_messages).
    pub is_pending: bool,
    /// Au moins un destinataire a ACKé.
    pub delivered: bool,
    /// Au moins un destinataire a lu.
    pub read: bool,
    /// Liste nominative — `None` en 1-à-1, `Some` pour groupe/Tous.
    pub detail: Option<ReceiptDetail>,
}

#[derive(Clone, Debug)]
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
    /// entre messages identiques ou vides (cas des médias).
    pub fn message_hash(msg: &ChatMessage) -> u64 {
        let key = format!(
            "{}:{}:{}:{}:{}",
            msg.from,
            msg.to_user.as_deref().unwrap_or("broadcast"),
            msg.timestamp_epoch.unwrap_or(0),
            msg.content,
            msg.media.as_ref().map(|m| m.id.as_str()).unwrap_or("")
        );
        let mut hash: u64 = 14_695_981_039_346_656_037;
        for byte in key.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
        }
        hash
    }

    /// Adresses des pairs qui doivent recevoir un ACK/ReadReceipt concernant `msg`.
    ///
    /// - message privé → seulement l'expéditeur
    /// - groupe (`#…`) → tous les membres en ligne (sauf soi)
    /// - diffusion (« Tous », `to_user == None`) → tous les pairs en ligne (sauf soi)
    ///
    /// Diffuser à tout le groupe permet à chaque membre d'accumuler le même état
    /// de réception/lecture et donc d'afficher le détail (« … ») pour tous.
    pub fn receipt_recipients(&self, msg: &ChatMessage) -> Vec<SocketAddr> {
        match &msg.to_user {
            // Privé : l'ACK ne repart que vers l'expéditeur.
            Some(u) if !u.starts_with('#') => self
                .peers
                .iter()
                .find(|p| p.username == msg.from && p.online)
                .map(|p| p.addr)
                .into_iter()
                .collect(),
            // Groupe : tous les membres en ligne (l'expéditeur en fait partie).
            Some(group) => {
                let members = self
                    .groups
                    .iter()
                    .find(|g| format!("#{}", g.name) == *group)
                    .map(|g| g.members.clone())
                    .unwrap_or_default();
                self.peers
                    .iter()
                    .filter(|p| {
                        p.online && p.username != self.my_username && members.contains(&p.username)
                    })
                    .map(|p| p.addr)
                    .collect()
            }
            // Diffusion « Tous » : tous les pairs en ligne.
            None => self
                .peers
                .iter()
                .filter(|p| p.online && p.username != self.my_username)
                .map(|p| p.addr)
                .collect(),
        }
    }

    /// Enregistre un ACK de livraison reçu d'un pair.
    pub fn mark_message_delivered_by(&mut self, message_hash: u64, username: String) {
        self.receipts
            .entry(message_hash)
            .or_default()
            .delivered_by
            .insert(username);
    }

    /// Enregistre un ReadReceipt reçu d'un pair.
    pub fn mark_message_read_by(&mut self, message_hash: u64, username: String) {
        self.receipts
            .entry(message_hash)
            .or_default()
            .read_by
            .insert(username);
    }

    /// Retourne l'état de réception calculé pour un message, avec ou sans détail nominatif.
    /// `with_detail = true` pour les groupes (popup •••), `false` pour les duos.
    pub fn get_receipt_state(&self, message_hash: u64, with_detail: bool) -> ReceiptState {
        let entry = self.receipts.get(&message_hash);
        let is_pending = self.pending_messages.contains_key(&message_hash);
        let delivered = entry.map(|e| !e.delivered_by.is_empty()).unwrap_or(false);
        let read = entry.map(|e| !e.read_by.is_empty()).unwrap_or(false);
        let detail = if with_detail {
            let mut dby: Vec<String> = entry
                .map(|e| e.delivered_by.iter().cloned().collect())
                .unwrap_or_default();
            let mut rby: Vec<String> = entry
                .map(|e| e.read_by.iter().cloned().collect())
                .unwrap_or_default();
            dby.sort();
            rby.sort();
            Some(ReceiptDetail {
                delivered_by: dby,
                read_by: rby,
            })
        } else {
            None
        };
        ReceiptState {
            is_pending,
            delivered,
            read,
            detail,
        }
    }

    /// Marque un message comme envoyé (en attente d'ACK, session uniquement)
    pub fn mark_message_sent(&mut self, message_hash: u64, to_addr: SocketAddr) {
        self.pending_messages.insert(
            message_hash,
            PendingMessage {
                to_addr,
                last_retry: SystemTime::now(),
                retry_count: 0,
            },
        );
    }

    /// Retire le message de la file d'attente (ACK reçu).
    pub fn mark_message_acked(&mut self, message_hash: u64) {
        self.pending_messages.remove(&message_hash);
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
mod tests {
    use crate::app::{AppState, Peer};
    use crate::message::{ChatMessage, Group};
    use std::net::SocketAddr;
    use std::time::SystemTime;

    fn state() -> AppState {
        let mut s = AppState::new("alice".to_string());
        s.messages.clear();
        s.peers.clear();
        s
    }

    fn make_msg(from: &str, content: &str) -> ChatMessage {
        ChatMessage {
            from: from.to_string(),
            content: content.to_string(),
            timestamp: "12:00".to_string(),
            timestamp_epoch: None,
            to_user: None,
            media: None,
        }
    }

    fn peer(name: &str, port: u16, online: bool) -> Peer {
        Peer {
            username: name.to_string(),
            addr: format!("127.0.0.1:{}", port).parse().unwrap(),
            last_seen: 0,
            online,
        }
    }

    fn addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{}", port).parse().unwrap()
    }

    #[test]
    fn test_message_hash_deterministic() {
        let m = make_msg("alice", "bonjour");
        assert_eq!(AppState::message_hash(&m), AppState::message_hash(&m));
    }

    #[test]
    fn test_message_hash_different_for_different_inputs() {
        let m1 = make_msg("alice", "hello");
        let m2 = make_msg("alice", "world");
        assert_ne!(AppState::message_hash(&m1), AppState::message_hash(&m2));
    }

    #[test]
    fn test_message_hash_differs_by_sender() {
        let m1 = make_msg("alice", "hello");
        let m2 = make_msg("bob", "hello");
        assert_ne!(AppState::message_hash(&m1), AppState::message_hash(&m2));
    }

    #[test]
    fn test_message_hash_stable_known_value() {
        let m = ChatMessage {
            from: "alice".to_string(),
            content: "bonjour".to_string(),
            timestamp: "12:00".to_string(),
            timestamp_epoch: Some(1_750_000_000),
            to_user: Some("bob".to_string()),
            media: None,
        };
        let expected = AppState::message_hash(&m);
        assert_eq!(AppState::message_hash(&m), expected);
        assert_ne!(expected, 0);
    }

    #[test]
    fn test_duplicate_content_different_epoch_gives_different_hash() {
        let m1 = ChatMessage {
            from: "alice".to_string(),
            content: "Bonjour".to_string(),
            timestamp: "14:00".to_string(),
            timestamp_epoch: Some(1_000),
            to_user: None,
            media: None,
        };
        let m2 = ChatMessage {
            timestamp_epoch: Some(2_000),
            ..m1.clone()
        };
        assert_ne!(AppState::message_hash(&m1), AppState::message_hash(&m2));
    }

    // ── receipt_recipients ─────────────────────────────────────────────────

    #[test]
    fn recipients_private_only_sender() {
        let mut s = state();
        s.peers = vec![peer("bob", 9001, true), peer("carol", 9002, true)];
        let mut m = make_msg("bob", "salut");
        m.to_user = Some("alice".to_string()); // privé vers moi
        assert_eq!(s.receipt_recipients(&m), vec![addr(9001)]);
    }

    #[test]
    fn recipients_broadcast_all_online_peers() {
        let mut s = state();
        s.peers = vec![
            peer("bob", 9001, true),
            peer("carol", 9002, true),
            peer("dan", 9003, false), // hors-ligne → exclu
        ];
        let m = make_msg("bob", "à tous"); // to_user None
        let mut got = s.receipt_recipients(&m);
        got.sort();
        assert_eq!(got, vec![addr(9001), addr(9002)]);
    }

    #[test]
    fn recipients_group_only_online_members() {
        let mut s = state();
        s.peers = vec![
            peer("bob", 9001, true),
            peer("carol", 9002, true),
            peer("eve", 9004, true), // pas membre → exclue
        ];
        s.groups = vec![Group {
            name: "team".to_string(),
            owner: "alice".to_string(),
            members: vec!["alice".to_string(), "bob".to_string(), "carol".to_string()],
            created_at: "2026-01-01 00:00:00".to_string(),
        }];
        let mut m = make_msg("bob", "coucou groupe");
        m.to_user = Some("#team".to_string());
        let mut got = s.receipt_recipients(&m);
        got.sort();
        assert_eq!(got, vec![addr(9001), addr(9002)]);
    }

    #[test]
    fn test_mark_delivered_by() {
        let mut s = state();
        let m = make_msg("alice", "test");
        let hash = AppState::message_hash(&m);
        s.mark_message_delivered_by(hash, "bob".to_string());
        let rs = s.get_receipt_state(hash, false);
        assert!(rs.delivered);
        assert!(!rs.read);
    }

    #[test]
    fn test_mark_read_by() {
        let mut s = state();
        let hash = AppState::message_hash(&make_msg("alice", "x"));
        s.mark_message_read_by(hash, "bob".to_string());
        s.mark_message_read_by(hash, "charlie".to_string());
        let rs = s.get_receipt_state(hash, false);
        assert!(rs.read);
    }

    #[test]
    fn test_receipt_detail_sorted() {
        let mut s = state();
        let hash = AppState::message_hash(&make_msg("alice", "y"));
        s.mark_message_delivered_by(hash, "zara".to_string());
        s.mark_message_delivered_by(hash, "alice".to_string());
        s.mark_message_read_by(hash, "bob".to_string());
        let rs = s.get_receipt_state(hash, true);
        let detail = rs.detail.unwrap();
        assert_eq!(detail.delivered_by, vec!["alice", "zara"]);
        assert_eq!(detail.read_by, vec!["bob"]);
    }

    #[test]
    fn test_no_detail_when_not_requested() {
        let mut s = state();
        let hash = AppState::message_hash(&make_msg("alice", "z"));
        s.mark_message_read_by(hash, "bob".to_string());
        let rs = s.get_receipt_state(hash, false);
        assert!(rs.detail.is_none());
    }

    #[test]
    fn test_mark_sent_and_is_pending() {
        let mut s = state();
        let hash = 42u64;
        let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
        assert!(!s.is_message_pending(hash));
        s.mark_message_sent(hash, addr);
        assert!(s.is_message_pending(hash));
    }

    #[test]
    fn test_mark_acked_removes_pending() {
        let mut s = state();
        let hash = 99u64;
        let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
        s.mark_message_sent(hash, addr);
        s.mark_message_acked(hash);
        assert!(!s.is_message_pending(hash));
    }

    #[test]
    fn test_pending_reflected_in_receipt_state() {
        let mut s = state();
        let hash = 55u64;
        let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
        s.mark_message_sent(hash, addr);
        let rs = s.get_receipt_state(hash, false);
        assert!(rs.is_pending);
        assert!(!rs.delivered);
        s.mark_message_acked(hash);
        let rs2 = s.get_receipt_state(hash, false);
        assert!(!rs2.is_pending);
    }

    #[test]
    fn test_get_retry_messages_increments_retry_count() {
        let mut s = state();
        let hash = 1u64;
        let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
        s.mark_message_sent(hash, addr);
        if let Some(p) = s.pending_messages.get_mut(&hash) {
            p.last_retry = SystemTime::UNIX_EPOCH;
        }
        let retries = s.get_retry_messages();
        assert!(!retries.is_empty());
        assert_eq!(retries[0].0, hash);
        assert_eq!(s.pending_messages[&hash].retry_count, 1);
    }

    #[test]
    fn test_get_retry_messages_empty_when_recent() {
        let mut s = state();
        let hash = 2u64;
        let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
        s.mark_message_sent(hash, addr);
        let retries = s.get_retry_messages();
        assert!(retries.is_empty());
    }
}
