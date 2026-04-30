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
