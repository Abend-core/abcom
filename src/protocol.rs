//! Limites partagées du protocole réseau.

/// Version du protocole filaire. Le projet n'ayant pas encore publié de
/// release, les versions incompatibles sont rejetées explicitement.
pub const PROTOCOL_VERSION: u16 = 1;

pub const MAX_USERNAME_CHARS: usize = 64;

/// Au-delà de 1 Gio, un transfert média nécessite l'accord du destinataire.
pub const MEDIA_ACK_THRESHOLD_BYTES: u64 = 1024 * 1024 * 1024;

/// Taille maximale absolue d'un transfert média individuel (2 Gio).
pub const MAX_MEDIA_TRANSFER_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub fn media_requires_ack(size_bytes: u64) -> bool {
    size_bytes > MEDIA_ACK_THRESHOLD_BYTES
}

pub fn valid_username(username: &str) -> bool {
    !username.trim().is_empty()
        && username == username.trim()
        && !username.starts_with('#')
        && username.chars().count() <= MAX_USERNAME_CHARS
        && !username.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_ack_threshold_is_strict() {
        assert!(!media_requires_ack(MEDIA_ACK_THRESHOLD_BYTES));
        assert!(media_requires_ack(MEDIA_ACK_THRESHOLD_BYTES + 1));
    }

    #[test]
    fn usernames_are_bounded_and_unambiguous() {
        assert!(valid_username("alice"));
        assert!(!valid_username(""));
        assert!(!valid_username(" alice"));
        assert!(!valid_username("#group"));
        assert!(!valid_username(&"a".repeat(MAX_USERNAME_CHARS + 1)));
    }
}
