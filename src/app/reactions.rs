use super::AppState;
use crate::message::{ChatMessage, ReactionAction, ReactionEntry, ReactionEvent};

impl AppState {
    /// Chemin LOCAL : l'utilisateur courant clique un emoji. Calcule lui-même
    /// s'il s'agit d'un ajout ou d'un retrait (toggle), met à jour l'état, et
    /// renvoie l'action nette à diffuser sur le réseau.
    pub fn toggle_reaction(
        &mut self,
        message_hash: u64,
        emoji: &str,
        user: &str,
    ) -> ReactionAction {
        let entries = self.reactions.entry(message_hash).or_default();
        let entry_idx = entries.iter().position(|e| e.emoji == emoji);
        let action = match entry_idx {
            Some(i) if entries[i].users.iter().any(|u| u == user) => {
                entries[i].users.retain(|u| u != user);
                if entries[i].users.is_empty() {
                    entries.remove(i);
                }
                ReactionAction::Remove
            }
            Some(i) => {
                entries[i].users.push(user.to_string());
                ReactionAction::Add
            }
            None => {
                entries.push(ReactionEntry {
                    emoji: emoji.to_string(),
                    users: vec![user.to_string()],
                });
                ReactionAction::Add
            }
        };
        let entries = self
            .reactions
            .get(&message_hash)
            .cloned()
            .unwrap_or_default();
        self.persist(super::StorageCmd::ReplaceReactions {
            hash: message_hash,
            entries,
        });
        self.bump_content();
        action
    }

    /// Chemin RÉSEAU : un événement explicite Add/Remove arrive d'un pair.
    /// Applique l'action donnée sans recalcul de toggle (idempotent : un Add
    /// déjà présent ou un Remove déjà absent ne change rien).
    pub fn apply_reaction_event(&mut self, event: &ReactionEvent) {
        let entries = self.reactions.entry(event.message_hash).or_default();
        match event.action {
            ReactionAction::Add => match entries.iter_mut().find(|e| e.emoji == event.emoji) {
                Some(e) if !e.users.iter().any(|u| u == &event.user) => {
                    e.users.push(event.user.clone())
                }
                Some(_) => {}
                None => entries.push(ReactionEntry {
                    emoji: event.emoji.clone(),
                    users: vec![event.user.clone()],
                }),
            },
            ReactionAction::Remove => {
                if let Some(e) = entries.iter_mut().find(|e| e.emoji == event.emoji) {
                    e.users.retain(|u| u != &event.user);
                }
                entries.retain(|e| !e.users.is_empty());
            }
        }
        let entries = self
            .reactions
            .get(&event.message_hash)
            .cloned()
            .unwrap_or_default();
        self.persist(super::StorageCmd::ReplaceReactions {
            hash: event.message_hash,
            entries,
        });
        self.bump_content();
    }

    pub fn reactions_for(&self, message_hash: u64) -> &[ReactionEntry] {
        self.reactions
            .get(&message_hash)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Recherche du message cible d'une réponse, dans l'historique en mémoire
    /// (ring buffer de 500). `None` si le message a expiré ou n'a jamais été
    /// reçu (repli UI : « message d'origine introuvable »).
    pub fn find_message_by_hash(&self, hash: u64) -> Option<&ChatMessage> {
        self.messages.iter().find(|m| Self::message_hash(m) == hash)
    }
}

#[cfg(test)]
#[path = "../tests/test_app_reactions.rs"]
mod tests;
