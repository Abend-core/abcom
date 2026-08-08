use super::AppState;
use crate::message::ChatMessage;

impl AppState {
    pub fn add_message(&mut self, msg: ChatMessage) {
        // Message entrant dans la conversation ouverte : marqué lu d'emblée.
        // La clé de lecture est le pair émetteur (privé) ou le salon (`#nom`).
        let read_key: Option<String> = match &self.selected_conversation {
            Some(conv) if conv.starts_with('#') => (msg.to_user.as_deref() == Some(conv.as_str())
                && msg.from != self.my_username)
                .then(|| conv.clone()),
            Some(user) => (msg.from == *user
                && msg.to_user.as_deref() == Some(self.my_username.as_str()))
            .then(|| user.clone()),
            None => None,
        };

        self.persist(super::StorageCmd::InsertMessage(msg.clone()));
        self.messages.push(msg);
        if let Some(key) = read_key {
            self.mark_conversation_read(&key);
        }
        // La fenêtre mémoire reste bornée ; l'historique complet vit en base
        // (les messages drainés restent chargeables par pagination).
        if self.messages.len() > self.history_cap() {
            let overflow = self.messages.len() - self.history_cap() + 99;
            let n = overflow.min(self.messages.len());
            self.messages.drain(0..n);
            self.oldest_loaded_rowid = None; // rowids inconnus après drain
            self.purge_stale_message_state();
        }
        self.bump_content();
    }

    /// Retire des maps annexes (réactions, accusés, en-attente) les entrées
    /// dont le message est sorti du ring-buffer — sans quoi elles croissent
    /// indéfiniment au fil de la session.
    fn purge_stale_message_state(&mut self) {
        let live: std::collections::HashSet<u64> =
            self.messages.iter().map(Self::message_hash).collect();
        self.reactions.retain(|hash, _| live.contains(hash));
        self.read_receipts.retain(|hash, _| live.contains(hash));
        self.delivered_receipts
            .retain(|hash, _| live.contains(hash));
        self.pending_messages.retain(|hash, _| live.contains(hash));
        self.failed_messages.retain(|hash, _| live.contains(hash));
    }

    /// Marque une conversation comme lue. `conv` est un nom de pair
    /// (conversation privée) ou une clé de salon `#nom` (groupe).
    pub fn mark_conversation_read(&mut self, conv: &str) {
        let count = self.incoming_message_count(conv);
        self.read_counts.insert(conv.to_string(), count);
        self.persist(super::StorageCmd::SetReadCount {
            username: conv.to_string(),
            count: count as u64,
        });
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
            // Salon de groupe : tous les messages adressés à la clé `#nom`,
            // quel qu'en soit l'auteur (y compris moi).
            Some(conv) if conv.starts_with('#') => self
                .messages
                .iter()
                .filter(|m| m.to_user.as_deref() == Some(conv.as_str()))
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

    /// Nombre de messages non-lus d'une conversation : nom de pair (privé)
    /// ou clé de salon `#nom` (groupe).
    pub fn unread_count(&self, conv: &str) -> usize {
        if self.selected_conversation.as_deref() == Some(conv) {
            return 0;
        }
        let total = self.incoming_message_count(conv);
        let read = *self.read_counts.get(conv).unwrap_or(&0);
        total.saturating_sub(read)
    }

    /// Clé d'un message entrant : le salon, ou l'expéditeur ; `None` pour les nôtres et « Tous ».
    fn incoming_key(&self, msg: &ChatMessage) -> Option<String> {
        let to = msg.to_user.as_deref()?;
        if msg.from == self.my_username {
            return None;
        }
        if to.starts_with('#') {
            Some(to.to_string())
        } else if to == self.my_username {
            Some(msg.from.clone())
        } else {
            None
        }
    }

    /// Total entrant d'une conversation : un seul parcours du ring-buffer par génération.
    fn incoming_message_count(&self, conv: &str) -> usize {
        let mut cache = self.incoming_counts.borrow_mut();
        if cache.0 != self.content_generation {
            cache.1.clear();
            for msg in &self.messages {
                if let Some(key) = self.incoming_key(msg) {
                    *cache.1.entry(key).or_insert(0) += 1;
                }
            }
            cache.0 = self.content_generation;
        }
        cache.1.get(conv).copied().unwrap_or(0)
    }

    pub fn clear_conversation_history(&mut self) {
        match &self.selected_conversation {
            None => self.messages.retain(|m| m.to_user.is_some()),
            Some(conv) if conv.starts_with('#') => {
                let key = conv.clone();
                self.messages
                    .retain(|m| m.to_user.as_deref() != Some(key.as_str()));
            }
            Some(username) => {
                let me = self.my_username.clone();
                let u = username.clone();
                self.messages.retain(|m| {
                    !((m.from == u && m.to_user.as_deref() == Some(me.as_str()))
                        || (m.from == me && m.to_user.as_deref() == Some(u.as_str())))
                });
            }
        }
        self.persist(super::StorageCmd::DeleteConversation {
            me: self.my_username.clone(),
            conv: self.selected_conversation.clone(),
        });
        self.bump_content();
    }
}

#[cfg(test)]
#[path = "../tests/test_app_messages.rs"]
mod tests;
