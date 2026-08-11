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

/// Futur qui ne rend son verdict qu'une fois la « porte » ouverte, comme un
/// sélecteur natif qui attend l'utilisateur.
struct Gated {
    gate: Arc<AtomicBool>,
    outcome: Option<super::PickerOutcome>,
    armed: bool,
}

impl std::future::Future for Gated {
    type Output = Option<super::PickerOutcome>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<super::PickerOutcome>> {
        if self.gate.load(Ordering::SeqCst) {
            return std::task::Poll::Ready(self.outcome.take());
        }
        if !self.armed {
            self.armed = true;
            let gate = self.gate.clone();
            let waker = cx.waker().clone();
            std::thread::spawn(move || {
                while !gate.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                waker.wake();
            });
        }
        std::task::Poll::Pending
    }
}

fn wait_until_closed() -> bool {
    for _ in 0..500 {
        if !super::is_open() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    false
}

/// Régression : rien n'empêchait un second clic d'ouvrir un deuxième sélecteur
/// natif par-dessus (ou, sous Windows, derrière la fenêtre) — d'où l'impression
/// qu'un clic ne fait rien et que le suivant se comporte bizarrement.
///
/// Les deux phases partagent un test : le drapeau est global au processus, des
/// tests concurrents se marcheraient dessus.
#[test]
fn only_one_picker_runs_at_a_time_and_cancelling_releases_it() {
    let ctx = egui::Context::default();
    let (tx, rx) = std::sync::mpsc::channel();

    // Phase 1 : sélecteur abandonné (aucun fichier choisi).
    assert!(!super::is_open());
    let gate = Arc::new(AtomicBool::new(false));
    super::spawn(
        tx.clone(),
        ctx.clone(),
        Gated {
            gate: gate.clone(),
            outcome: None,
            armed: false,
        },
    );
    assert!(super::is_open(), "le sélecteur ouvert doit être signalé");

    gate.store(true, Ordering::SeqCst);
    assert!(wait_until_closed(), "l'abandon doit relâcher le drapeau");
    assert!(rx.try_recv().is_err(), "un abandon n'envoie aucun verdict");

    // Phase 2 : sélecteur qui rend un verdict.
    let gate = Arc::new(AtomicBool::new(true));
    super::spawn(
        tx,
        ctx,
        Gated {
            gate,
            outcome: Some(super::PickerOutcome::Export(std::path::PathBuf::from(
                "conversation.txt",
            ))),
            armed: false,
        },
    );

    let received = rx.recv_timeout(std::time::Duration::from_secs(2));
    assert!(matches!(
        received,
        Ok(super::PickerOutcome::Export(ref path)) if path.ends_with("conversation.txt")
    ));
    assert!(wait_until_closed(), "le verdict rendu relâche le drapeau");
}
