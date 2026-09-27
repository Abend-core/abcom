#![cfg_attr(not(feature = "sound"), allow(dead_code))]

// Sans la feature `sound`, `rodio` (donc ALSA sous Linux) n'est pas compilé :
// le bip devient un no-op et rien d'autre ne change.
#[cfg(not(feature = "sound"))]
pub(crate) fn play_notification_sound() {}

#[cfg(feature = "sound")]
pub(crate) use enabled::play_notification_sound;

#[cfg(feature = "sound")]
mod enabled {
    use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
    use std::sync::OnceLock;
    use std::time::Duration;

    use rodio::{DeviceSinkBuilder, Player, Source};

    /// Canal vers le thread audio pérenne, créé au premier bip. Un seul thread et
    /// une seule initialisation du périphérique pour toute la vie du processus —
    /// l'ancienne version créait un thread + un `OutputStream` par notification.
    static AUDIO_TX: OnceLock<SyncSender<()>> = OnceLock::new();

    /// Joue deux tonalités courtes (880 Hz puis 1100 Hz) sans bloquer le thread
    /// UI. Les demandes en rafale sont coalescées (canal borné à 1 : un bip en
    /// file d'attente au plus).
    pub(crate) fn play_notification_sound() {
        let tx = AUDIO_TX.get_or_init(|| {
            let (tx, rx) = sync_channel::<()>(1);
            std::thread::Builder::new()
                .name("abcom-audio".into())
                .spawn(move || audio_loop(rx))
                .ok();
            tx
        });
        let _ = tx.try_send(());
    }

    /// Boucle du thread audio : périphérique ouvert une seule fois, réutilisé
    /// pour chaque notification.
    fn audio_loop(rx: Receiver<()>) {
        // rodio ≥ 0.21 : le périphérique est ouvert via un builder et expose un
        // mixeur ; les `Sink` d'autrefois sont des `Player` branchés dessus.
        let Ok(device) = DeviceSinkBuilder::open_default_sink() else {
            // Pas de périphérique audio : on draine silencieusement les demandes.
            while rx.recv().is_ok() {}
            return;
        };
        while rx.recv().is_ok() {
            let player = Player::connect_new(device.mixer());
            let tone1 = rodio::source::SineWave::new(880.0)
                .take_duration(Duration::from_millis(80))
                .amplify(0.15);
            let tone2 = rodio::source::SineWave::new(1100.0)
                .take_duration(Duration::from_millis(80))
                .amplify(0.15);
            player.append(tone1);
            player.append(tone2);
            player.sleep_until_end();
        }
    }
}
