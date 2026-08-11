use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Futur qui reste en attente jusqu'à ce qu'un autre thread le réveille :
/// c'est le comportement d'un sélecteur natif, qui ne rend son verdict qu'au
/// clic de l'utilisateur.
struct WokenElsewhere {
    ready: Arc<AtomicBool>,
    armed: bool,
}

impl std::future::Future for WokenElsewhere {
    type Output = u32;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<u32> {
        if self.ready.load(Ordering::SeqCst) {
            return std::task::Poll::Ready(42);
        }
        if !self.armed {
            self.armed = true;
            let ready = self.ready.clone();
            let waker = cx.waker().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(20));
                ready.store(true, Ordering::SeqCst);
                waker.wake();
            });
        }
        std::task::Poll::Pending
    }
}

#[test]
fn a_pending_future_is_awaited_until_another_thread_wakes_it() {
    let future = WokenElsewhere {
        ready: Arc::new(AtomicBool::new(false)),
        armed: false,
    };
    assert_eq!(super::block_on(future), 42);
}

#[test]
fn an_already_finished_future_returns_without_parking() {
    assert_eq!(super::block_on(std::future::ready(7)), 7);
}
