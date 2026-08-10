use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use abcom::platform::{autostart, notify};
use abcom::util::MutexExt;
use abcom::{app, config, discovery, identity, message, network, protocol, ui};

/// mimalloc rend les pages à l'OS, ce que l'allocateur système ne fait pas au repli dans le tray.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Seules variables lues depuis `.env` : une liste fermée évite toute injection.
const DOTENV_KEYS: [&str; 3] = ["ABCOM_KLIPY_API_KEY", "ABCOM_PASSPHRASE", "ABCOM_INSTANCE"];

/// Délai borné laissé aux tâches réseau à la fermeture : une fenêtre fermée doit disparaître.
const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// `CLE=valeur`, `#` en commentaire, `export` et guillemets tolérés ; clés inconnues ignorées.
fn parse_dotenv(content: &str) -> Vec<(&str, &str)> {
    let mut pairs = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_start_matches("export ").trim();
        if !DOTENV_KEYS.contains(&key) {
            tracing::debug!("clé .env ignorée : {key}");
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        pairs.push((key, value));
    }
    pairs
}

/// Charge le `.env` ; une variable déjà définie prime toujours sur le fichier.
fn load_dotenv(path: &str) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    for (key, value) in parse_dotenv(&content) {
        if std::env::var_os(key).is_some() {
            continue;
        }
        // SAFETY : appelé avant tout spawn, aucun autre fil ne lit l'environnement.
        std::env::set_var(key, value);
    }
}

/// Journalisation : console **et** fichier tournant dans le répertoire de données.
fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;
    use tracing_subscriber::Layer as _;

    let filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "abcom=info".into())
    };

    // Le fichier est un supplément : si le répertoire n'est pas accessible,
    // on garde la console plutôt que de perdre toute journalisation.
    let dir = config::data_dir().join("logs");
    let file = std::fs::create_dir_all(&dir).ok().map(|()| {
        let (writer, guard) =
            tracing_appender::non_blocking(tracing_appender::rolling::daily(&dir, "abcom.log"));
        let layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(writer)
            .with_filter(filter());
        (layer, guard)
    });
    let (file_layer, guard) = match file {
        Some((layer, guard)) => (Some(layer), Some(guard)),
        None => (None, None),
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(filter()))
        .with(file_layer)
        .init();
    guard
}

/// Écrit la cause d'une panique sur disque : en release (strippé, sans console) rien ne s'affiche.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let dir = config::data_dir();
        let report = format!(
            "abcom {} — {}\n{info}\n",
            env!("CARGO_PKG_VERSION"),
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(dir.join("last-panic.txt"), &report);
        }
        tracing::error!("panique : {info}");
        previous(info);
    }));
}

fn main() -> anyhow::Result<()> {
    load_dotenv(".env");
    // Le garde doit vivre aussi longtemps que le processus, sinon les
    // dernières lignes ne sont jamais écrites sur disque.
    let _log_guard = init_logging();
    install_panic_hook();

    let username = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "anonymous".to_string())
    });
    if !protocol::valid_username(&username) {
        anyhow::bail!(
            "pseudo invalide : 1 à {} caractères, sans espace extérieur, contrôle ni préfixe #",
            protocol::MAX_USERNAME_CHARS
        );
    }

    // Contexte egui partagé avec les tâches de fond : renseigné au lancement
    // de l'UI, il permet de la réveiller à chaque événement (cf. notify.rs).
    let ui_ctx: notify::UiContext = Arc::new(std::sync::OnceLock::new());

    let (send_tx, send_rx) = mpsc::channel::<message::NetworkSendRequest>(512);
    let (send_media_tx, send_media_rx) = mpsc::channel::<message::MediaSendJob>(64);

    let media_dir = config::data_dir().join("media");

    // Runtime tokio multi-thread — tourne en arrière-plan pendant qu'egui
    // occupe le thread principal. Deux workers suffisent largement : les
    // tâches sont du réseau/disque bufferisé, pas du calcul.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    // Canaux vers l'UI : chaque événement relayé réveille egui.
    let (event_tx, event_rx) = notify::ui_channel::<message::AppEvent>(256, ui_ctx.clone(), &rt);
    let (media_offer_tx, media_offer_rx) =
        notify::ui_channel::<message::MediaStreamOffer>(16, ui_ctx.clone(), &rt);

    // Stockage SQLite : ouverture (migration JSON au premier lancement),
    // chargement de la fenêtre récente, puis thread d'écriture dédié.
    let storage = app::Storage::open(&config::data_dir())
        .map_err(|e| anyhow::anyhow!("ouverture du stockage : {e}"))?;
    let loaded = storage
        .load_all(app::storage::INITIAL_WINDOW)
        .map_err(|e| anyhow::anyhow!("chargement du stockage : {e}"))?;
    let referenced_media = match storage.all_media_ids() {
        Ok(ids) => Some(ids),
        Err(error) => {
            tracing::error!(
                "GC cache désactivé : lecture des médias référencés impossible : {error}"
            );
            None
        }
    };
    let storage_tx = app::storage::spawn(storage, event_tx.clone());

    // Identité cryptographique locale + magasin de confiance TOFU : toutes
    // les connexions (chat et médias) sont chiffrées Noise XX.
    let local_identity = identity::Identity::load_or_create(&config::data_dir())?;
    let identity_fingerprint = local_identity.fingerprint();
    let trust = Arc::new(network::secure::TrustStore::new(
        loaded.peer_keys.clone(),
        Some(storage_tx.clone()),
    ));
    // Passphrase de salon optionnelle (variable ABCOM_PASSPHRASE, chargeable
    // depuis .env) : durcit le handshake en XXpsk3 — tous les pairs du salon
    // doivent la partager.
    let psk = std::env::var("ABCOM_PASSPHRASE")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .map(|p| network::secure::derive_psk(p.trim()));
    let psk_active = psk.is_some();
    if psk_active {
        tracing::info!("passphrase de salon active (handshake XXpsk3)");
    }

    let net_ctx = Arc::new(network::NetContext {
        identity: local_identity.clone(),
        username: username.clone(),
        trust: trust.clone(),
        event_tx: event_tx.clone(),
        psk,
    });
    let pool = network::ConnectionPool::new(net_ctx.clone());

    let state = Arc::new(Mutex::new(app::AppState::new(
        username.clone(),
        loaded,
        Some(storage_tx),
    )));

    // Autostart : activé par défaut au premier lancement d'un build release
    // (préférence persistée, interrupteur dans Paramètres).
    {
        let mut s = state.lock_safe();
        let existing = s.kv.get("autostart").map(|v| v == "1");
        let effective = autostart::init_default(existing);
        if existing.is_none() {
            s.set_pref("autostart", if effective { "1" } else { "0" });
        }
    }

    // GC du cache disque des médias (orphelins + plafond), hors chemin de
    // démarrage : l'UI s'ouvre sans attendre le parcours du dossier.
    if let Some(referenced_media) = referenced_media {
        let dir = media_dir.clone();
        std::thread::spawn(move || app::media::gc_media_dir(dir, referenced_media));
    }

    // Pair expiré → sa connexion est libérée : son adresse ne sera plus valide.
    let (peer_gone_tx, mut peer_gone_rx) = mpsc::channel::<String>(64);
    {
        let pool = pool.clone();
        rt.spawn(async move {
            while let Some(username) = peer_gone_rx.recv().await {
                pool.drop_peer(&username).await;
            }
        });
    }
    rt.spawn(pool.clone().sweep_closed());

    rt.spawn(discovery::run(
        username.clone(),
        local_identity.clone(),
        event_tx.clone(),
        peer_gone_tx,
    ));
    rt.spawn(network::run_server(net_ctx.clone()));
    rt.spawn(network::run_sender(send_rx, pool.clone()));
    rt.spawn(network::run_media_sender(send_media_rx, net_ctx.clone()));
    rt.spawn(network::run_media_server(
        net_ctx.clone(),
        media_offer_tx,
        media_dir,
    ));

    ui::run(
        state,
        ui_ctx,
        identity_fingerprint,
        psk_active,
        ui::UiRuntimeChannels {
            event_rx,
            event_tx: event_tx.clone(),
            send_tx,
            send_media_tx,
            media_offer_rx,
            trust,
        },
    )?;

    // Le flush SQLite est fait ; on laisse les tâches réseau finir leurs trames en cours.
    tracing::info!("arrêt : purge des tâches réseau en cours");
    rt.shutdown_timeout(SHUTDOWN_GRACE);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_dotenv;

    #[test]
    fn parses_quotes_comments_and_ignores_unknown_keys() {
        let content = concat!(
            "# commentaire\n",
            "\n",
            "ABCOM_KLIPY_API_KEY=\"abc 123\"\n",
            "export ABCOM_PASSPHRASE='secret'\n",
            "  ABCOM_INSTANCE = 2 \n",
            "PATH=/usr/bin\n",
            "ligne sans egal\n",
        );
        assert_eq!(
            parse_dotenv(content),
            [
                ("ABCOM_KLIPY_API_KEY", "abc 123"),
                ("ABCOM_PASSPHRASE", "secret"),
                ("ABCOM_INSTANCE", "2"),
            ]
        );
    }

    #[test]
    fn keeps_inner_quotes_when_unbalanced() {
        assert_eq!(
            parse_dotenv("ABCOM_PASSPHRASE=\"toujours ouvert"),
            [("ABCOM_PASSPHRASE", "\"toujours ouvert")]
        );
    }
}
