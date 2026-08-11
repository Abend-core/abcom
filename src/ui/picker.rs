//! Sélecteurs de fichiers natifs, sans blocage du thread de rendu.
//!
//! Les variantes bloquantes de `rfd` appellent `runModal` (macOS) ou son
//! équivalent : une **boucle d'événements imbriquée** démarre à l'intérieur de
//! celle de winit, qui se retrouve à traiter un événement alors qu'elle en
//! traite déjà un et panique — l'application disparaissait sans un mot, au
//! hasard des événements en vol, à l'ouverture de « joindre un fichier ».
//!
//! Les variantes asynchrones présentent une feuille rattachée à la fenêtre et
//! rendent la main tout de suite. On attend leur verdict sur un thread dédié,
//! et il revient à l'UI par un canal qui la réveille.

use std::future::Future;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Ce qu'un sélecteur natif a rendu, appliqué à la frame suivante.
pub(crate) enum PickerOutcome {
    /// Fichiers ou dossier à joindre au message en cours.
    Attachments(Vec<PathBuf>),
    /// Destination d'un export de conversation.
    Export(PathBuf),
    /// Image de profil choisie.
    Avatar(PathBuf),
}

/// Présente un sélecteur et rend son résultat par `tx`, sans rien bloquer.
///
/// `dialog` est construit par l'appelant, sur le thread de l'UI : c'est là que
/// la fenêtre native apparaît. Seule l'attente part sur un thread dédié.
pub(crate) fn spawn<F>(tx: Sender<PickerOutcome>, ctx: egui::Context, dialog: F)
where
    F: Future<Output = Option<PickerOutcome>> + Send + 'static,
{
    let spawned = std::thread::Builder::new()
        .name("abcom-picker".into())
        .spawn(move || {
            if let Some(outcome) = block_on(dialog) {
                if tx.send(outcome).is_ok() {
                    ctx.request_repaint();
                }
            }
        });
    if spawned.is_err() {
        tracing::error!("sélecteur de fichiers : thread indisponible");
    }
}

/// Attend un futur sur le thread courant.
///
/// Les sélecteurs sont les seuls futurs de l'interface : leur donner accès au
/// runtime tokio des tâches réseau les coupleraient sans rien apporter, une
/// attente par parking suffit.
fn block_on<F: Future>(future: F) -> F::Output {
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            // `park` peut rendre la main sans réveil : la boucle re-sonde.
            Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
#[path = "../tests/test_ui_picker.rs"]
mod tests;
