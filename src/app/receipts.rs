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
