/// Identifiant métier d'une conversation.
///
/// La représentation historique de l'UI et du stockage reste compatible :
/// `None` pour « Tous », un username pour un échange privé et `#nom` pour un
/// groupe. Le reste du domaine ne doit plus réinterpréter directement ces
/// chaînes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConversationId {
    Broadcast,
    Peer(String),
    Group(String),
}

impl ConversationId {
    pub fn from_key(key: Option<&str>) -> Self {
        match key {
            None => Self::Broadcast,
            Some(key) => match key.strip_prefix('#') {
                Some(group) => Self::Group(group.to_string()),
                None => Self::Peer(key.to_string()),
            },
        }
    }

    pub fn message_target(&self) -> Option<String> {
        match self {
            Self::Broadcast => None,
            Self::Peer(username) => Some(username.clone()),
            Self::Group(name) => Some(format!("#{name}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConversationId;

    #[test]
    fn converts_legacy_keys_at_the_boundary() {
        assert_eq!(ConversationId::from_key(None), ConversationId::Broadcast);
        assert_eq!(
            ConversationId::from_key(Some("bob")),
            ConversationId::Peer("bob".into())
        );
        assert_eq!(
            ConversationId::from_key(Some("#team")),
            ConversationId::Group("team".into())
        );
    }

    #[test]
    fn produces_wire_compatible_targets() {
        assert_eq!(ConversationId::Broadcast.message_target(), None);
        assert_eq!(
            ConversationId::Peer("bob".into())
                .message_target()
                .as_deref(),
            Some("bob")
        );
        assert_eq!(
            ConversationId::Group("team".into())
                .message_target()
                .as_deref(),
            Some("#team")
        );
    }
}
