//! Réveil de l'UI par événement : les tâches tokio écrivent dans un canal
//! relais dont la sortie notifie egui (`request_repaint`) à chaque message.
//! La boucle de rendu n'a ainsi plus besoin de repeindre périodiquement pour
//! dépiler les canaux — au repos, aucun repaint n'est déclenché.

use std::sync::{Arc, OnceLock};

use tokio::sync::mpsc;

/// Contexte egui partagé avec les tâches d'arrière-plan. Vide au démarrage,
/// renseigné à la création de l'application (`ui::run`), après quoi chaque
/// événement relayé réveille la boucle de rendu.
pub type UiContext = Arc<OnceLock<egui::Context>>;

/// Crée un canal mpsc dont chaque message relayé vers le récepteur déclenche
/// un `request_repaint` : l'émetteur s'utilise comme un `mpsc::Sender`
/// ordinaire, le récepteur est dépilé par l'UI à la frame suivante.
pub fn ui_channel<T: Send + 'static>(
    capacity: usize,
    ctx: UiContext,
    rt: &tokio::runtime::Runtime,
) -> (mpsc::Sender<T>, mpsc::Receiver<T>) {
    let (in_tx, mut in_rx) = mpsc::channel::<T>(capacity);
    let (out_tx, out_rx) = mpsc::channel::<T>(capacity);
    rt.spawn(async move {
        while let Some(value) = in_rx.recv().await {
            if out_tx.send(value).await.is_err() {
                break;
            }
            if let Some(ctx) = ctx.get() {
                ctx.request_repaint();
            }
        }
    });
    (in_tx, out_rx)
}
