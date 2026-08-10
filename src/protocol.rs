//! Limites partagées du protocole réseau.

/// Version du protocole filaire ; les versions incompatibles sont rejetées.
///
/// 2 : annonces de découverte signées (Ed25519 dérivé de l'identité Noise).
pub const PROTOCOL_VERSION: u16 = 2;

pub const MAX_USERNAME_CHARS: usize = 64;

/// Au-delà de ce seuil, un transfert média nécessite l'accord du destinataire.
///
/// 50 Mio couvre les documents et photos réels (un gros PDF ou Word tourne
/// autour de 5–50 Mo, une photo haute résolution de 5–25 Mo) tout en bornant
/// ce qu'un pair peut écrire sans confirmation : avec `MAX_CONCURRENT_RECEIVES`
/// réceptions simultanées, le pire cas silencieux reste de l'ordre de 200 Mio
/// au lieu de plusieurs Gio.
pub const MEDIA_ACK_THRESHOLD_BYTES: u64 = 50 * 1024 * 1024;

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
