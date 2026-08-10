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
        let Some(last) = self.last_incoming_hash(conv) else {
            return;
        };
        self.read_marks.insert(conv.to_string(), last);
        self.persist(super::StorageCmd::SetReadMark {
            username: conv.to_string(),
            message_hash: last,
        });
        self.bump_content();
    }

    /// Hash du dernier message entrant d'une conversation.
    fn last_incoming_hash(&self, conv: &str) -> Option<u64> {
        self.messages
            .iter()
            .rev()
            .find(|m| self.incoming_key(m).as_deref() == Some(conv))
            .map(Self::message_hash)
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
    /// Non-lus d'une conversation : messages entrants postérieurs au dernier
    /// marqué lu.
    ///
    /// Repère par **hash de message** et non par compteur : après une purge du
    /// ring-buffer ou un effacement d'historique, un compteur pouvait désigner
    /// un tout autre ensemble de messages.
    pub fn unread_count(&self, conv: &str) -> usize {
        if self.selected_conversation.as_deref() == Some(conv) {
            return 0;
        }
        let mark = self.read_marks.get(conv).copied();
        let mut unread = 0;
        // On remonte le fil : tout ce qui suit le repère est non lu.
        for msg in self.messages.iter().rev() {
            if self.incoming_key(msg).as_deref() != Some(conv) {
                continue;
            }
            if Some(Self::message_hash(msg)) == mark {
                return unread;
            }
            unread += 1;
        }
        unread
    }

    /// Ce message est-il déjà dans la fenêtre mémoire ?
    ///
    /// Un ACK perdu fait réémettre l'expéditeur jusqu'à cinq fois : sans ce
    /// contrôle, chaque réémission créait une copie de plus.
    pub fn has_message(&self, message_hash: u64) -> bool {
        self.messages
            .iter()
            .any(|m| Self::message_hash(m) == message_hash)
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
