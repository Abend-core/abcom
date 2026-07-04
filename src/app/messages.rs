use super::AppState;
use crate::message::ChatMessage;

impl AppState {
    pub fn add_message(&mut self, msg: ChatMessage) {
        let incoming_from_selected = self
            .selected_conversation
            .as_ref()
            .map(|u| msg.from == *u && msg.to_user.as_deref() == Some(self.my_username.as_str()))
            .unwrap_or(false);
        let from = msg.from.clone();

        self.messages.push(msg);
        if incoming_from_selected {
            self.mark_conversation_read(&from);
        }
        if self.messages.len() > 500 {
            self.messages.drain(0..100);
            self.purge_stale_message_state();
        }
        self.dirty.messages = true;
        self.bump_content();
    }

    /// Retire des maps annexes (réactions, accusés, en-attente) les entrées
    /// dont le message est sorti du ring-buffer — sans quoi elles croissent
    /// indéfiniment au fil de la session.
    fn purge_stale_message_state(&mut self) {
        let live: std::collections::HashSet<u64> =
            self.messages.iter().map(Self::message_hash).collect();
        let before = self.reactions.len();
        self.reactions.retain(|hash, _| live.contains(hash));
        if self.reactions.len() != before {
            self.dirty.reactions = true;
        }
        self.read_receipts.retain(|hash, _| live.contains(hash));
        self.pending_messages.retain(|hash, _| live.contains(hash));
    }

    pub fn mark_conversation_read(&mut self, peer_username: &str) {
        let me = self.my_username.as_str();
        let count = self
            .messages
            .iter()
            .filter(|m| m.from == peer_username && m.to_user.as_deref() == Some(me))
            .count();
        self.read_counts.insert(peer_username.to_string(), count);
        self.dirty.read_counts = true;
        self.bump_content();
    }

    /// Messages de la conversation sélectionnée
    pub fn get_conversation_messages(&self) -> Vec<&ChatMessage> {
        match &self.selected_conversation {
            None => self
                .messages
                .iter()
                .filter(|m| m.to_user.is_none())
                .collect(),
            Some(username) => self
                .messages
                .iter()
                .filter(|m| {
                    (m.from == *username && m.to_user == Some(self.my_username.clone()))
                        || (m.from == self.my_username && m.to_user == Some(username.clone()))
                })
                .collect(),
        }
    }

    #[allow(dead_code)]
    pub fn get_conversations(&self) -> Vec<String> {
        let mut convos = vec!["📢 Global".to_string()];
        for peer in &self.peers {
            convos.push(format!("🙋 {}", peer.username));
        }
        convos
    }

    pub fn unread_count(&self, peer_username: &str) -> usize {
        if self.selected_conversation.as_ref() == Some(&peer_username.to_string()) {
            return 0;
        }
        let total = self
            .messages
            .iter()
            .filter(|m| m.from == peer_username && m.to_user == Some(self.my_username.clone()))
            .count();
        let read = *self.read_counts.get(peer_username).unwrap_or(&0);
        total.saturating_sub(read)
    }

    pub fn clear_conversation_history(&mut self) {
        match &self.selected_conversation {
            None => self.messages.retain(|m| m.to_user.is_some()),
            Some(username) => {
                let me = self.my_username.clone();
                let u = username.clone();
                self.messages.retain(|m| {
                    !((m.from == u && m.to_user.as_deref() == Some(me.as_str()))
                        || (m.from == me && m.to_user.as_deref() == Some(u.as_str())))
                });
            }
        }
        self.dirty.messages = true;
        self.bump_content();
    }
}

#[cfg(test)]
#[path = "../tests/test_app_messages.rs"]
mod tests;
