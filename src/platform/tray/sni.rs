//! Icône résidente Linux via StatusNotifierItem, le protocole D-Bus que les
//! bureaux implémentent réellement (KDE, XFCE, GNOME avec l'extension
//! AppIndicator). `libappindicator` n'était qu'une enveloppe GTK autour de ce
//! protocole : le parler directement retire GTK3, libappindicator et libxdo des
//! dépendances de compilation comme d'exécution.

use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, TrayMethods as _};
use tokio::sync::mpsc;

use super::{queue, TrayAction, TRAY_PX};

/// Délai laissé au bureau pour accepter l'icône avant de conclure à l'absence
/// de zone de notification.
const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

struct AbcomTray {
    normal: Icon,
    badge: Icon,
    unread: bool,
    open_label: String,
    quit_label: String,
}

impl ksni::Tray for AbcomTray {
    fn id(&self) -> String {
        "abcom".into()
    }

    fn title(&self) -> String {
        "Abcom".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Communications
    }

    /// `NeedsAttention` sur non-lus : les bureaux qui masquent les icônes au
    /// repos font réapparaître celles qui réclament l'attention.
    fn status(&self) -> ksni::Status {
        if self.unread {
            ksni::Status::NeedsAttention
        } else {
            ksni::Status::Active
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let icon = if self.unread {
            &self.badge
        } else {
            &self.normal
        };
        vec![icon.clone()]
    }

    /// Clic gauche sur l'icône : convention Linux, ouvrir la fenêtre.
    fn activate(&mut self, _x: i32, _y: i32) {
        queue(TrayAction::Open);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: self.open_label.clone(),
                activate: Box::new(|_: &mut Self| queue(TrayAction::Open)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: self.quit_label.clone(),
                activate: Box::new(|_: &mut Self| queue(TrayAction::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Convertit une image RGBA en ARGB gros-boutiste, seul format accepté par la
/// spécification StatusNotifierItem.
fn to_argb(mut rgba: Vec<u8>) -> Icon {
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.rotate_right(1);
    }
    Icon {
        width: TRAY_PX as i32,
        height: TRAY_PX as i32,
        data: rgba,
    }
}

/// Monte l'icône sur un thread dédié et renvoie de quoi lui pousser l'état du
/// badge. `None` si le bureau n'offre pas de zone de notification : l'appelant
/// retombe alors sur « fermer la fenêtre quitte ».
pub(super) fn spawn(open_label: String, quit_label: String) -> Option<mpsc::UnboundedSender<bool>> {
    let (normal, badged) = super::icon_rgba()?;
    let tray = AbcomTray {
        normal: to_argb(normal),
        badge: to_argb(badged),
        unread: false,
        open_label,
        quit_label,
    };

    let (unread_tx, mut unread_rx) = mpsc::unbounded_channel::<bool>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("abcom-tray-sni".into())
        .spawn(move || {
            // Runtime dédié : le thread reste bloqué dedans, ce qui fait
            // tourner les tâches D-Bus de ksni pour toute la vie du processus.
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(e) => {
                    tracing::warn!("tray : runtime indisponible ({e})");
                    let _ = ready_tx.send(false);
                    return;
                }
            };

            runtime.block_on(async move {
                let handle = match tray.spawn().await {
                    Ok(handle) => handle,
                    Err(e) => {
                        tracing::warn!("tray : StatusNotifier indisponible ({e})");
                        let _ = ready_tx.send(false);
                        return;
                    }
                };
                let _ = ready_tx.send(true);
                tracing::info!("tray : icône StatusNotifier posée");

                while let Some(unread) = unread_rx.recv().await {
                    if handle.update(|tray| tray.unread = unread).await.is_none() {
                        tracing::warn!("tray : service arrêté, badge non appliqué");
                        break;
                    }
                }
            });
        })
        .ok()?;

    // Le thread répond dès que le bureau a accepté l'icône ; au-delà, on
    // considère qu'il n'y a pas de zone de notification.
    match ready_rx.recv_timeout(READY_TIMEOUT) {
        Ok(true) => Some(unread_tx),
        Ok(false) => None,
        Err(e) => {
            tracing::warn!("tray : pas de réponse du bureau ({e})");
            None
        }
    }
}
