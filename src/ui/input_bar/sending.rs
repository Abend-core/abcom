//! Envoi : préparation des médias, découpage en flux et validation du message.

use crate::ui::i18n;
use std::path::Path;

use crate::app::AppState;
use crate::message::{ChatMessage, MediaAttachment, MediaKind, MediaSendJob, MediaStreamHeader};
use crate::protocol::{media_requires_ack, MAX_MEDIA_TRANSFER_BYTES};
use crate::util::MutexExt;

use std::sync::{Arc, Mutex};

use crate::ui::AbcomApp;

/// Threads de préparation média tournant en parallèle.
///
/// La préparation est bornée par les E/S (copie, zip), pas par le processeur :
/// en multiplier les threads n'accélère rien. Aligné sur le nombre de
/// réceptions simultanées côté réseau.
const MEDIA_PREP_WORKERS: usize = 4;

type PrepJob = Box<dyn FnOnce() + Send>;

/// File de préparation média, servie par un pool de taille fixe.
///
/// Un thread par pièce jointe faisait exploser le nombre de threads OS dès
/// qu'on sélectionnait un dossier entier — des centaines de threads se
/// disputant le même disque, pour un débit moindre.
fn prep_pool() -> &'static std::sync::mpsc::Sender<PrepJob> {
    static POOL: std::sync::OnceLock<std::sync::mpsc::Sender<PrepJob>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<PrepJob>();
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..MEDIA_PREP_WORKERS {
            let rx = rx.clone();
            std::thread::spawn(move || {
                loop {
                    // Le verrou est relâché avant d'exécuter la tâche, sinon
                    // le pool se comporterait comme un thread unique.
                    let job = rx.lock_safe().recv();
                    match job {
                        Ok(job) => job(),
                        Err(_) => break,
                    }
                }
            });
        }
        tx
    })
}

/// Envoie un fichier (ou un dossier zippé) comme média, par streaming.
///
/// Tout le travail lourd (zip, copie locale dans `media/<id>`, lecture) part
/// dans le pool de préparation : l'UI ne gèle jamais, même pour plusieurs Go.
pub(super) fn send_one_media(
    app: &AbcomApp,
    path: &Path,
    my_name: &str,
    to_user: &Option<String>,
    targets: &[(String, std::net::SocketAddr)],
) {
    let state = app.state.clone();
    let send_media_tx = app.net.send_media_tx.clone();
    let path = path.to_path_buf();
    let my_name = my_name.to_string();
    let to_user = to_user.clone();
    let targets = targets.to_vec();

    let job: PrepJob = Box::new(move || {
        if let Err(e) =
            prepare_and_stream(&state, &send_media_tx, &path, &my_name, &to_user, &targets)
        {
            tracing::warn!("préparation média échouée ({}): {}", path.display(), e);
        }
    });
    if prep_pool().send(job).is_err() {
        tracing::error!("pool de préparation média arrêté, pièce jointe ignorée");
    }
}

/// Prépare un média dans `media/<id>` (copie d'un fichier ou zip d'un dossier),
/// l'ajoute à notre historique, puis met en file un envoi vers chaque pair.
pub(super) fn prepare_and_stream(
    state: &Arc<Mutex<AppState>>,
    send_media_tx: &tokio::sync::mpsc::Sender<MediaSendJob>,
    path: &Path,
    my_name: &str,
    to_user: &Option<String>,
    targets: &[(String, std::net::SocketAddr)],
) -> std::io::Result<()> {
    let is_dir = path.is_dir();
    let filename = crate::ui::media::media_display_name(path);
    let id = crate::ui::media::media_id(&filename);

    // Un fichier n'est plus recopié dans `media/` : l'original est déjà sur la
    // machine, le dupliquer coûtait autant d'octets pour rien. Un dossier, lui,
    // n'a pas d'original — son archive doit bien être écrite quelque part.
    let dest = if is_dir {
        let archive = state.lock_safe().media_path(&id);
        if let Some(parent) = archive.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::archive::zip_dir_to_path(path, &archive)?;
        archive
    } else {
        path.to_path_buf()
    };

    let size_bytes = std::fs::metadata(&dest)?.len();
    if size_bytes > MAX_MEDIA_TRANSFER_BYTES {
        // Ne jamais supprimer le fichier de l'utilisateur : seule une archive
        // que nous venons de fabriquer nous appartient.
        if is_dir {
            let _ = std::fs::remove_file(&dest);
        }
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "média trop volumineux (maximum 2 Gio)",
        ));
    }
    let (kind, width, height) = if !is_dir && MediaAttachment::is_image_filename(&filename) {
        let dims = image::image_dimensions(&dest).ok();
        (MediaKind::Image, dims.map(|d| d.0), dims.map(|d| d.1))
    } else {
        (MediaKind::File, None, None)
    };

    let media = MediaAttachment {
        id,
        filename,
        kind,
        size_bytes,
        url: None,
        width,
        height,
    };
    let now = chrono::Local::now();
    let header = MediaStreamHeader {
        from: my_name.to_string(),
        to_user: to_user.clone(),
        timestamp: now.format("%H:%M").to_string(),
        timestamp_epoch: Some(now.timestamp() as u64),
        media: media.clone(),
        requires_ack: media_requires_ack(size_bytes),
    };

    let jobs: Vec<MediaSendJob> = targets
        .iter()
        .map(|(username, addr)| MediaSendJob {
            to_peer: username.clone(),
            to_addr: *addr,
            source_path: dest.clone(),
            header: header.clone(),
        })
        .collect();
    let mut permits = Vec::with_capacity(jobs.len());
    for _ in &jobs {
        match send_media_tx.try_reserve() {
            Ok(permit) => permits.push(permit),
            Err(error) => {
                if is_dir {
                    let _ = std::fs::remove_file(&dest);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    format!("mise en file du média impossible : {error}"),
                ));
            }
        }
    }

    // Où trouver ce fichier chez nous : sans cette note, notre propre fil
    // n'aurait plus ni vignette ni ouverture, faute de copie dans `media/`.
    {
        let mut state = state.lock_safe();
        if !is_dir {
            state.register_local_media(media.id.clone(), dest.clone());
        }
        // Notre propre copie du message (la carte apparaît, avec progression).
        state.add_message(ChatMessage {
            from: my_name.to_string(),
            content: String::new(),
            timestamp: header.timestamp.clone(),
            timestamp_epoch: header.timestamp_epoch,
            to_user: to_user.clone(),
            media: Some(media),
            reply_to: None,
            // Pas de nonce : le destinataire reconstruit ce message depuis
            // MediaStreamHeader et doit retomber sur le même hash.
            nonce: None,
        });
    }

    for (permit, job) in permits.into_iter().zip(jobs) {
        permit.send(job);
    }
    Ok(())
}

/// Taille filaire d'un message de chat : le JSON de l'enveloppe
/// `NetworkPacket::Chat`, tel qu'il passera dans le canal chiffré.
pub(super) fn chat_wire_size(msg: &ChatMessage) -> usize {
    serde_json::to_vec(&crate::message::NetworkPacket::Chat(msg.clone()))
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}

pub(super) fn send_current_message(app: &mut AbcomApp) -> bool {
    let has_message = !app.composer.text.trim().is_empty();
    let has_attachments = !app.composer.pending_attachments.is_empty();
    if !has_message && !has_attachments {
        return false;
    }

    let (my_name, selected_peer_name, transfer_targets) = {
        let s = app.state.lock_safe();
        (
            s.my_username.clone(),
            s.selected_conversation_id().message_target(),
            s.selected_transfer_targets(),
        )
    };

    if has_attachments && transfer_targets.is_empty() {
        app.last_notification = Some(app.t(i18n::AUCUN_DESTINATAIRE_EN_LIGNE_POUR_L).to_string());
        app.notification_time = std::time::Instant::now();
        return false;
    }

    if has_message {
        if app.composer.text.ends_with('\n') {
            app.composer.text.pop();
        }

        let content = app.composer.text.trim().to_string();
        let now = chrono::Local::now();
        let msg = ChatMessage {
            from: my_name.clone(),
            content,
            timestamp: now.format("%H:%M").to_string(),
            timestamp_epoch: Some(now.timestamp() as u64),
            to_user: selected_peer_name.clone(),
            media: None,
            reply_to: app.replying_to.as_ref().map(|r| r.message_hash),
            nonce: Some(ChatMessage::fresh_nonce()),
        };

        // La réception coupe la connexion au-delà de MAX_LOGICAL_MESSAGE :
        // refuser ici avec un retour clair plutôt que perdre le message
        // silencieusement (l'input est conservé).
        if chat_wire_size(&msg) > crate::network::secure::MAX_LOGICAL_MESSAGE {
            app.last_notification = Some(
                app.t(i18n::MESSAGE_TROP_VOLUMINEUX_POUR_ETRE_ENVOYE)
                    .to_string(),
            );
            app.notification_time = std::time::Instant::now();
            return false;
        }

        if !app.enqueue_chat_message(msg) {
            return false;
        }
    }

    if has_attachments {
        // Chemin unique pour tout fichier ou dossier : streaming par morceaux.
        let targets: Vec<(String, std::net::SocketAddr)> = transfer_targets
            .iter()
            .map(|t| (t.username.clone(), t.addr))
            .collect();

        for path in app.composer.pending_attachments.clone() {
            send_one_media(app, &path, &my_name, &selected_peer_name, &targets);
        }
    }

    app.composer.text.clear();
    app.composer.cursor_char = 0;
    app.composer.has_focus = true;
    app.composer.scroll_lines = 0.0;
    app.composer.pending_attachments.clear();
    app.replying_to = None;

    true
}
