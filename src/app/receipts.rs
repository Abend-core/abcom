use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::time::SystemTime;

use crate::message::ChatMessage;
use super::AppState;

/// Message en attente d'ACK
#[derive(Clone, Debug)]
pub struct PendingMessage {
    pub message_hash: u64,
    pub to_addr: SocketAddr,
    pub last_retry: SystemTime,
    pub retry_count: u32,
}

impl AppState {
    /// Calcule un hash de message pour identifier les accusés de lecture
    pub fn message_hash(msg: &ChatMessage) -> u64 {
        let content = format!("{}:{}", msg.from, msg.content);
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    pub fn mark_message_read(&mut self, message_hash: u64, username: String) {
        self.read_receipts
            .entry(message_hash)
            .or_insert_with(std::collections::HashSet::new)
            .insert(username);
    }

    pub fn is_message_read_by(&self, message_hash: u64, username: &str) -> bool {
        self.read_receipts
            .get(&message_hash)
            .map(|r| r.contains(username))
            .unwrap_or(false)
    }

    pub fn get_read_count(&self, message_hash: u64) -> usize {
        self.read_receipts.get(&message_hash).map(|r| r.len()).unwrap_or(0)
    }

    /// Marque un message comme envoyé (en attente d'ACK)
    pub fn mark_message_sent(&mut self, message_hash: u64, to_addr: SocketAddr) {
        self.pending_messages.insert(message_hash, PendingMessage {
            message_hash,
            to_addr,
            last_retry: SystemTime::now(),
            retry_count: 0,
        });
    }

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

    pub fn is_message_pending(&self, message_hash: u64) -> bool {
        self.pending_messages.contains_key(&message_hash)
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::SystemTime;
    use crate::app::AppState;
    use crate::message::ChatMessage;

    fn state() -> AppState {
        let mut s = AppState::new("alice".to_string());
        s.messages.clear();
        s.peers.clear();
        s
    }

    fn make_msg(from: &str, content: &str) -> ChatMessage {
        ChatMessage { from: from.to_string(), content: content.to_string(), timestamp: "12:00".to_string(), to_user: None }
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
    fn test_mark_and_check_read() {
        let mut s = state();
        let m = make_msg("alice", "test");
        let hash = AppState::message_hash(&m);
        s.mark_message_read(hash, "bob".to_string());
        assert!(s.is_message_read_by(hash, "bob"));
        assert!(!s.is_message_read_by(hash, "charlie"));
    }

    #[test]
    fn test_get_read_count() {
        let mut s = state();
        let hash = AppState::message_hash(&make_msg("alice", "x"));
        assert_eq!(s.get_read_count(hash), 0);
        s.mark_message_read(hash, "bob".to_string());
        s.mark_message_read(hash, "charlie".to_string());
        assert_eq!(s.get_read_count(hash), 2);
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
    fn test_get_retry_messages_increments_retry_count() {
        let mut s = state();
        let hash = 1u64;
        let addr: SocketAddr = "192.168.1.1:9000".parse().unwrap();
        // Set last_retry far in the past to force immediate retry
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
        s.mark_message_sent(hash, addr); // last_retry = now
        let retries = s.get_retry_messages();
        // retry_count=0 → delay=1s, elapsed<1s → no retry yet
        assert!(retries.is_empty());
    }
}
