use std::sync::{Mutex, MutexGuard};

/// Verrouillage tolérant à l'empoisonnement : si un thread a paniqué en
/// tenant le verrou, on récupère la donnée plutôt que de propager la panique
/// (les mutations sont toutes courtes, l'état reste cohérent au grain où on
/// le lit).
pub trait MutexExt<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Dimensions au-delà desquelles une image reçue est refusée sans être décodée.
///
/// `image` plafonne l'allocation à 512 Mo mais ne borne pas les dimensions : un
/// fichier de quelques kilo-octets peut donc nous faire allouer beaucoup. On lit
/// l'en-tête d'abord, on décode seulement si c'est raisonnable.
pub const MAX_IMAGE_SIDE: u32 = 8192;

/// Décode une image reçue du réseau, en refusant les dimensions déraisonnables.
pub fn decode_image_bounded(bytes: &[u8]) -> Option<image::DynamicImage> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    if width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        tracing::warn!("image refusée : {width}x{height} dépasse {MAX_IMAGE_SIDE}");
        return None;
    }
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_safe_recovers_after_poisoning() {
        let mutex = Mutex::new(0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = mutex.lock().unwrap();
            panic!("boom");
        }));
        assert!(result.is_err());
        assert!(mutex.is_poisoned());

        let guard = mutex.lock_safe();
        assert_eq!(*guard, 0);
    }
}
