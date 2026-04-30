use crate::message::ChatMessage;
use super::AppState;

impl AppState {
    pub fn add_message(&mut self, msg: ChatMessage) {
        let incoming_from_selected = self.selected_conversation.as_ref().map(|u| {
            msg.from == *u && msg.to_user == Some(self.my_username.clone())
        }).unwrap_or(false);

        self.messages.push(msg.clone());
        if incoming_from_selected {
            self.mark_conversation_read(&msg.from);
        }
        if self.messages.len() > 500 {
            self.messages.drain(0..100);
        }
        self.save_messages();
    }

    pub fn mark_conversation_read(&mut self, peer_username: &str) {
        let count = self.messages.iter().filter(|m| {
            m.from == peer_username && m.to_user == Some(self.my_username.clone())
        }).count();
        self.read_counts.insert(peer_username.to_string(), count);
        self.save_read_counts();
    }

    /// Messages de la conversation sélectionnée
    pub fn get_conversation_messages(&self) -> Vec<&ChatMessage> {
        match &self.selected_conversation {
            None => self.messages.iter().filter(|m| m.to_user.is_none()).collect(),
            Some(username) => self.messages.iter().filter(|m| {
                (m.from == *username && m.to_user == Some(self.my_username.clone()))
                    || (m.from == self.my_username && m.to_user == Some(username.clone()))
            }).collect(),
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
        let total = self.messages.iter().filter(|m| {
            m.from == peer_username && m.to_user == Some(self.my_username.clone())
        }).count();
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
                    !((m.from == u && m.to_user == Some(me.clone()))
                        || (m.from == me && m.to_user == Some(u.clone())))
                });
            }
        }
        self.save_messages();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::AppState;
    use crate::message::ChatMessage;

    fn state(username: &str) -> AppState {
        let mut s = AppState::new(username.to_string());
        s.messages.clear();
        s.peers.clear();
        s.read_counts.clear();
        s
    }

    fn msg(from: &str, to: Option<&str>, content: &str) -> ChatMessage {
        ChatMessage {
            from: from.to_string(),
            content: content.to_string(),
            timestamp: "12:00".to_string(),
            to_user: to.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_add_message_increases_count() {
        let mut s = state("alice");
        s.add_message(msg("bob", None, "hello"));
        assert_eq!(s.messages.len(), 1);
    }

    #[test]
    fn test_unread_count_zero_no_messages() {
        let s = state("alice");
        assert_eq!(s.unread_count("bob"), 0);
    }

    #[test]
    fn test_unread_count_increments() {
        let mut s = state("alice");
        s.messages.push(msg("bob", Some("alice"), "hi"));
        s.messages.push(msg("bob", Some("alice"), "hey"));
        assert_eq!(s.unread_count("bob"), 2);
    }

    #[test]
    fn test_unread_count_zero_when_conversation_selected() {
        let mut s = state("alice");
        s.messages.push(msg("bob", Some("alice"), "hi"));
        s.selected_conversation = Some("bob".to_string());
        assert_eq!(s.unread_count("bob"), 0);
    }

    #[test]
    fn test_mark_conversation_read_clears_unread() {
        let mut s = state("alice");
        s.messages.push(msg("bob", Some("alice"), "hi"));
        s.messages.push(msg("bob", Some("alice"), "hey"));
        s.mark_conversation_read("bob");
        assert_eq!(s.unread_count("bob"), 0);
    }

    #[test]
    fn test_get_broadcast_messages() {
        let mut s = state("alice");
        s.messages.push(msg("bob", None, "broadcast"));
        s.messages.push(msg("bob", Some("alice"), "private"));
        // selected_conversation = None → broadcast only
        let result = s.get_conversation_messages();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "broadcast");
    }

    #[test]
    fn test_get_private_conversation_messages() {
        let mut s = state("alice");
        s.messages.push(msg("bob", Some("alice"), "coucou"));
        s.messages.push(msg("alice", Some("bob"), "salut"));
        s.messages.push(msg("charlie", Some("alice"), "hey"));
        s.selected_conversation = Some("bob".to_string());
        let result = s.get_conversation_messages();
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|m| m.from == "bob" || m.from == "alice"));
    }

    #[test]
    fn test_clear_conversation_history_private() {
        let mut s = state("alice");
        s.messages.push(msg("bob", Some("alice"), "hi"));
        s.messages.push(msg("alice", Some("bob"), "ok"));
        s.messages.push(msg("charlie", Some("alice"), "hey"));
        s.selected_conversation = Some("bob".to_string());
        s.clear_conversation_history();
        // only charlie's message survives
        assert_eq!(s.messages.len(), 1);
        assert_eq!(s.messages[0].from, "charlie");
    }

    #[test]
    fn test_clear_conversation_history_broadcast() {
        let mut s = state("alice");
        s.messages.push(msg("bob", None, "global"));
        s.messages.push(msg("bob", Some("alice"), "private"));
        // No selection → clear broadcast
        s.clear_conversation_history();
        assert_eq!(s.messages.len(), 1);
        assert_eq!(s.messages[0].to_user, Some("alice".to_string()));
    }

    #[test]
    fn test_message_cap_at_500() {
        let mut s = state("alice");
        // Fill 500 messages then add 1 → drain 100 from front
        for i in 0..500 {
            s.messages.push(msg("bob", None, &i.to_string()));
        }
        s.add_message(msg("bob", None, "overflow"));
        assert_eq!(s.messages.len(), 401);
        assert_eq!(s.messages.last().unwrap().content, "overflow");
    }
}
