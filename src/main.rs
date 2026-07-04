use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

mod app;
mod archive;
mod autostart;
mod config;
mod discovery;
mod emoji_registry;
mod identity;
mod klipy;
mod message;
mod network;
mod notify;
mod ui;

fn main() -> anyhow::Result<()> {
    if let Ok(content) = std::fs::read_to_string(".env") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                std::env::set_var(k.trim(), v.trim());
            }
        }
    }

    let username = std::env::args().nth(1).unwrap_or_else(|| {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "anonymous".to_string())
    });

    // Contexte egui partagé avec les tâches de fond : renseigné au lancement
    // de l'UI, il permet de la réveiller à chaque événement (cf. notify.rs).
    let ui_ctx: notify::UiContext = Arc::new(std::sync::OnceLock::new());

    let (send_tx, send_rx) = mpsc::channel::<message::SendRequest>(256);
    let (send_group_tx, send_group_rx) = mpsc::channel::<message::SendGroupRequest>(256);
    let (send_typing_tx, send_typing_rx) = mpsc::channel::<message::TypingRequest>(256);
    let (send_read_receipt_tx, send_read_receipt_rx) =
        mpsc::channel::<message::ReadReceiptRequest>(256);
    let (send_ack_tx, send_ack_rx) = mpsc::channel::<message::MessageAckRequest>(256);
    let (send_avatar_tx, send_avatar_rx) = mpsc::channel::<message::AvatarRequest>(64);
    let (send_reaction_tx, send_reaction_rx) = mpsc::channel::<message::ReactionRequest>(256);
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
    let (event_tx, event_rx) =
        notify::ui_channel::<message::AppEvent>(256, ui_ctx.clone(), &rt);
    let (media_offer_tx, media_offer_rx) =
        notify::ui_channel::<message::MediaStreamOffer>(16, ui_ctx.clone(), &rt);

    // Stockage SQLite : ouverture (migration JSON au premier lancement),
    // chargement de la fenêtre récente, puis thread d'écriture dédié.
    let storage = app::Storage::open(&config::data_dir())
        .map_err(|e| anyhow::anyhow!("ouverture du stockage : {e}"))?;
    let loaded = storage.load_all(app::storage::INITIAL_WINDOW);
    let referenced_media = storage.all_media_ids().unwrap_or_default();
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
        eprintln!("[secure] Passphrase de salon active (handshake XXpsk3)");
    }

    let net_ctx = Arc::new(network::NetContext {
        identity: local_identity.clone(),
        username: username.clone(),
        trust,
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
        let mut s = state.lock().unwrap();
        let existing = s.kv.get("autostart").map(|v| v == "1");
        let effective = autostart::init_default(existing);
        if existing.is_none() {
            s.set_pref("autostart", if effective { "1" } else { "0" });
        }
    }

    // GC du cache disque des médias (orphelins + plafond), hors chemin de
    // démarrage : l'UI s'ouvre sans attendre le parcours du dossier.
    {
        let dir = media_dir.clone();
        std::thread::spawn(move || app::media::gc_media_dir(dir, referenced_media));
    }

    rt.spawn(discovery::run(
        username.clone(),
        local_identity.public_hex(),
        event_tx.clone(),
    ));
    rt.spawn(network::run_server(net_ctx.clone()));
    rt.spawn(network::run_sender(send_rx, pool.clone()));
    rt.spawn(network::run_sender_group(send_group_rx, pool.clone()));
    rt.spawn(network::run_sender_typing(send_typing_rx, pool.clone()));
    rt.spawn(network::run_sender_read_receipts(
        send_read_receipt_rx,
        pool.clone(),
    ));
    rt.spawn(network::run_sender_ack(send_ack_rx, pool.clone()));
    rt.spawn(network::run_sender_avatar(send_avatar_rx, pool.clone()));
    rt.spawn(network::run_sender_reaction(send_reaction_rx, pool.clone()));
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
        event_rx,
        send_tx,
        send_group_tx,
        send_typing_tx,
        send_read_receipt_tx,
        send_ack_tx,
        send_avatar_tx,
        send_reaction_tx,
        send_media_tx,
        media_offer_rx,
    )?;

    Ok(())
}
