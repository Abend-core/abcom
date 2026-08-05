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
