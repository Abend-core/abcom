use super::i18n;
use super::{sound::play_notification_sound, AbcomApp};
use crate::app::AppState;
use crate::message::{
    AppEvent, AvatarRequest, ChatMessage, GroupAction, GroupEvent, MessageAck, MessageAckRequest,
    NetworkSendRequest, ReadReceipt, ReadReceiptRequest, SendGroupRequest, SendRequest,
};
use crate::protocol::media_requires_ack;
use crate::util::MutexExt;

/// Accusés retenus en attente de commit. Plafond de sécurité : si la
/// persistance tombe durablement, on cesse d'accumuler.
const MAX_PENDING_ACKS: usize = 10_000;

impl AbcomApp {
    /// Dépile les événements réseau reçus depuis les tâches tokio.
    ///
    /// **Invariant de verrouillage** : rien n'est émis sur le réseau tant que
    /// le verrou d'état est tenu. Les requêtes sont accumulées puis envoyées
    /// après la boucle — le `drop`/relock manuel qui existait ici était la
    /// source classique de deadlock à la première modification venue.
    pub(crate) fn process_events(&mut self) {
        let mut outbound: Vec<NetworkSendRequest> = Vec::new();
        let mut outbox_flush: Vec<(u64, SendRequest)> = Vec::new();
        let mut s = self.state.lock_safe();
        while let Ok(evt) = self.net.event_rx.try_recv() {
            match evt {
                AppEvent::MessageReceived(msg) => {
                    // Message de salon : `to_user` porte la clé `#nom`. Un salon
                    // inconnu ou quitté est ignoré (l'émetteur ne devrait plus
                    // nous cibler ; protège aussi d'un pair non membre).
                    let group_conv: Option<String> = msg
                        .to_user
                        .as_deref()
                        .filter(|t| t.starts_with('#'))
                        .map(str::to_string);
                    if let Some(conv) = &group_conv {
                        let known = conv
                            .strip_prefix('#')
                            .and_then(|name| s.get_group(name))
                            .is_some_and(|group| {
                                group.members.contains(&s.my_username)
                                    && group.members.contains(&msg.from)
                            });
                        if !known {
                            continue;
                        }
                    }

                    // ACK automatique (livraison) pour tout message reçu d'un
                    // autre pair — privé, salon (#…) ou diffusion (« Tous »).
                    // En salon/« Tous », l'accusé est diffusé à tous les membres
                    // (pas seulement l'expéditeur) pour que chacun puisse
                    // consulter le détail « … » reçu/lu. Le ReadReceipt (lecture)
                    // n'est envoyé que si la conversation est déjà ouverte et la
                    // fenêtre active — sinon différé à l'ouverture.
                    if msg.from != s.my_username {
                        let recipients = s.receipt_recipients(&msg);
                        if !recipients.is_empty() {
                            let msg_hash = AppState::message_hash(&msg);
                            let now = chrono::Local::now().format("%H:%M").to_string();

                            // Conversation source côté destinataire : « Tous »
                            // (None), le salon (#…) ou le pair émetteur.
                            let source_conv: Option<String> = match &msg.to_user {
                                None => None,
                                Some(t) if t.starts_with('#') => Some(t.clone()),
                                Some(_) => Some(msg.from.clone()),
                            };
                            let already_reading =
                                self.window_focused && s.selected_conversation == source_conv;

                            let mut ack_reqs = Vec::with_capacity(recipients.len());
                            let mut receipt_reqs = Vec::new();
                            for (recipient, addr) in recipients {
                                ack_reqs.push(MessageAckRequest {
                                    to_peer: recipient.clone(),
                                    to_addr: addr,
                                    ack: MessageAck {
                                        from: s.my_username.clone(),
                                        to: recipient.clone(),
                                        message_hash: msg_hash,
                                        timestamp: now.clone(),
                                    },
                                });
                                if already_reading {
                                    receipt_reqs.push(ReadReceiptRequest {
                                        to_peer: recipient.clone(),
                                        to_addr: addr,
                                        receipt: ReadReceipt {
                                            from: s.my_username.clone(),
                                            to: recipient,
                                            message_hash: msg_hash,
                                            timestamp: now.clone(),
                                        },
                                    });
                                }
                            }

                            // Déjà en base (réémission) : rien n'attend, on
                            // acquitte tout de suite. Sinon l'accusé patiente
                            // jusqu'au commit — le dire « reçu » avant serait
                            // un mensonge qu'un arrêt brutal démentirait.
                            if s.has_message(msg_hash) || !s.has_storage() {
                                outbound.extend(ack_reqs.into_iter().map(Into::into));
                                // Mémorisé comme envoyé : sinon le rattrapage
                                // au retour du focus réémettrait tout le fil.
                                for req in &receipt_reqs {
                                    self.read_receipts_sent
                                        .entry(req.to_peer.clone())
                                        .or_default()
                                        .insert(msg_hash);
                                }
                                outbound.extend(receipt_reqs.into_iter().map(Into::into));
                            } else if self.pending_acks.len() < MAX_PENDING_ACKS {
                                let waiting: Vec<NetworkSendRequest> = ack_reqs
                                    .into_iter()
                                    .map(Into::into)
                                    .chain(receipt_reqs.into_iter().map(Into::into))
                                    .collect();
                                self.pending_acks
                                    .entry(msg_hash)
                                    .or_default()
                                    .extend(waiting);
                            } else {
                                // Persistance durablement en panne : on cesse
                                // d'accumuler plutôt que d'enfler sans fin.
                                tracing::warn!("accusés en attente saturés, message non acquitté");
                            }
                        }
                    }

                    // Doublon d'une réémission : l'ACK vient d'être traité
                    // ci-dessus, il ne reste qu'à ne pas dupliquer le message.
                    if s.has_message(AppState::message_hash(&msg)) {
                        tracing::debug!("message déjà reçu ignoré (réémission)");
                        continue;
                    }

                    s.add_message(msg.clone());
                    if msg.from != s.my_username {
                        self.last_notification = Some(match &group_conv {
                            Some(conv) => format!("{} · {}: {}", conv, msg.from, msg.content),
                            None => format!("{}: {}", msg.from, msg.content),
                        });
                        self.notification_time = std::time::Instant::now();
                        self.has_unread = true;
                        // Sourdine et « déjà ouverte » : la conversation source
                        // est le salon pour un message de groupe, le pair sinon.
                        let source_conv: Option<String> = if msg.to_user.is_none() {
                            None
                        } else if let Some(conv) = &group_conv {
                            Some(conv.clone())
                        } else {
                            Some(msg.from.clone())
                        };
                        let already_in_conv = s.selected_conversation == source_conv;
                        let conv_muted = self.muted_conversations.contains(&source_conv);
                        if self.window_hidden {
                            // Fenêtre repliée : notification système native
                            // (aperçu selon la préférence), pas de bip interne.
                            if !conv_muted {
                                Self::notify_native(
                                    msg.from.clone(),
                                    self.native_body_for(&msg.content),
                                );
                            }
                        } else if self.enable_sound_notifications && !already_in_conv && !conv_muted
                        {
                            play_notification_sound();
                        }
                    }
                }
                AppEvent::PeerDiscovered { username, addr } => {
                    s.add_peer(username.clone(), addr);
                    // Re-synchronise auprès du pair (ré)apparu les groupes dont
                    // nous sommes propriétaire et dont il est membre : il reçoit
                    // l'état complet (création manquée hors-ligne, ajout, etc.).
                    let sync_events: Vec<GroupEvent> = s
                        .groups
                        .iter()
                        .filter(|g| g.owner == s.my_username && g.members.contains(&username))
                        .map(|g| GroupEvent {
                            action: GroupAction::Create { group: g.clone() },
                        })
                        .collect();
                    for event in sync_events {
                        self.net.try_send(SendGroupRequest {
                            to_peer: username.clone(),
                            to_addr: addr,
                            event,
                        });
                    }
                    // Messages écrits pendant son absence : réémis maintenant.
                    let queued = s.outbox_for(&username);
                    if !queued.is_empty() {
                        let requests: Vec<_> = queued
                            .into_iter()
                            .map(|(hash, message)| {
                                (
                                    hash,
                                    SendRequest {
                                        to_peer: username.clone(),
                                        to_addr: addr,
                                        message,
                                    },
                                )
                            })
                            .collect();
                        outbox_flush.extend(requests);
                    }

                    // Première découverte (depuis le dernier envoi) : on partage
                    // notre avatar pour qu'il s'affiche chez ce pair.
                    if !self.avatar_sent_to.contains(&username) {
                        if let Some(announce) = s.avatar_announce() {
                            let request = AvatarRequest {
                                to_peer: username.clone(),
                                to_addr: addr,
                                announce,
                            };
                            if self.net.try_send(request) {
                                self.avatar_sent_to.insert(username);
                            }
                        }
                    }
                }
                AppEvent::PeerDisconnected { username } => {
                    s.mark_peer_offline(&username);
                    // Réémettre l'avatar à la prochaine reconnexion de ce pair.
                    self.avatar_sent_to.remove(&username);
                    // Idem accusés de lecture : il a pu manquer ceux émis pendant son absence.
                    self.read_receipts_sent.remove(&username);
                }
                AppEvent::UserTyping(username) => s.set_user_typing(username),
                AppEvent::GroupEventReceived { peer, event } => {
                    apply_group_event(&mut s, &peer, event);
                }
                AppEvent::ReadReceiptReceived(receipt) => {
                    if s.is_expected_receipt_sender(receipt.message_hash, &receipt.from) {
                        s.mark_message_read(receipt.message_hash, receipt.from.clone());
                    }
                }
                AppEvent::MessageAckReceived(ack) => {
                    if s.is_expected_ack_sender(ack.message_hash, &ack.from) {
                        s.mark_message_acked(ack.message_hash, &ack.from);
                        // Détail nominatif « reçu par » (popup « … » des salons).
                        s.mark_message_delivered_by(ack.message_hash, ack.from.clone());
                    }
                }
                AppEvent::ReactionReceived(event) => {
                    s.apply_reaction_event(&event);
                }
                AppEvent::KeyChanged {
                    username,
                    offered_key,
                } => {
                    // Clé non concordante : connexion refusée, la modale offre le ré-appairage.
                    self.modals.key_mismatch = Some((username.clone(), offered_key));
                    let label = self.t(i18n::LA_CLE_D_IDENTITE_A_CHANGE);
                    self.last_notification = Some(format!("⚠️ {} : {}", username, label));
                    self.notification_time = std::time::Instant::now();
                    if self.window_hidden {
                        Self::notify_native(format!("⚠️ {username}"), label.to_string());
                    } else if self.enable_sound_notifications {
                        play_notification_sound();
                    }
                }
                AppEvent::SearchResults { query, messages } => {
                    // Résultat d'une frappe plus ancienne : la requête a changé
                    // entre-temps, on ignore.
                    if query == self.search.submitted {
                        self.search.results = messages;
                    }
                }
                AppEvent::StorageUsage(usage) => {
                    self.storage_usage = Some(usage);
                    self.storage_scan_pending = false;
                }
                AppEvent::MediaPurged(report) => {
                    self.purge_preview_pending = false;
                    if report.dry_run {
                        self.purge_preview = Some(report);
                    } else {
                        // Des fichiers ont disparu : le fil doit le montrer.
                        self.media.presence.clear();
                        self.media.textures.clear();
                        self.media.texture_lru.clear();
                        // La ventilation affichée date d'avant la purge : on la
                        // redemande, sinon l'utilisateur voit le total inchangé.
                        // Via le verrou déjà tenu : le reprendre ici gelait
                        // l'application (cf. l'invariant en tête de fonction).
                        self.purge_preview = None;
                        s.request_storage_usage();
                        self.last_notification = Some(format!(
                            "{} — {} ({})",
                            self.t(i18n::PURGE_TERMINEE),
                            super::settings::human_bytes(report.freed_bytes),
                            report.freed_files
                        ));
                        self.notification_time = std::time::Instant::now();
                    }
                }
                AppEvent::SendFailed { username } => {
                    // Sans ceci, l'échec disparaît dans les logs d'un binaire sans console.
                    let label = self.t(i18n::INJOIGNABLE_MESSAGE_NON_ENVOYE);
                    self.last_notification = Some(format!("{username} : {label}"));
                    self.notification_time = std::time::Instant::now();
                }
                AppEvent::MessagesPersisted { hashes } => {
                    for hash in hashes {
                        if let Some(waiting) = self.pending_acks.remove(&hash) {
                            outbound.extend(waiting);
                        }
                    }
                }
                AppEvent::ConversationExported { error } => {
                    self.last_notification = Some(match error {
                        None => self.t(i18n::CONVERSATION_EXPORTEE).to_string(),
                        Some(error) => format!("{} : {error}", self.t(i18n::EXPORT_IMPOSSIBLE)),
                    });
                    self.notification_time = std::time::Instant::now();
                }
                AppEvent::MediaDownloaded { filename } => {
                    self.last_notification = Some(match filename {
                        Some(name) => format!("{} {name}", self.t(i18n::TELECHARGE)),
                        None => self.t(i18n::TELECHARGEMENT_IMPOSSIBLE).to_string(),
                    });
                    self.notification_time = std::time::Instant::now();
                }
                AppEvent::OlderMessagesLoaded {
                    messages,
                    oldest_rowid,
                } => {
                    // Page d'historique demandée par le scroll vers le haut :
                    // préfixée à la fenêtre mémoire, la compensation d'offset
                    // du fil évite tout saut visuel.
                    if messages.is_empty() {
                        // Début de l'historique atteint : ne plus rien attendre.
                        self.chat_prepend_fix = None;
                    } else {
                        self.chat_visible_count += messages.len();
                    }
                    s.prepend_older_messages(messages, oldest_rowid);
                    self.loading_older = false;
                }
                AppEvent::AvatarReceived(announce) => {
                    let from = announce.from.clone();
                    s.set_peer_avatar(announce.from, announce.png);
                    // Forcer le rechargement de la texture mise en cache.
                    self.avatar_textures.remove(&from);
                }
                AppEvent::MediaIncoming(header) => {
                    // Début de réception d'un média : on crée le message (carte
                    // + progression) ; les octets arrivent dans media/<id>.
                    let from = header.from.clone();
                    let msg = ChatMessage {
                        from: header.from,
                        content: String::new(),
                        timestamp: header.timestamp,
                        timestamp_epoch: header.timestamp_epoch,
                        to_user: header.to_user,
                        media: Some(header.media),
                        reply_to: None,
                        // Pas de nonce : le hash doit coïncider avec celui de
                        // la copie locale de l'émetteur (cf. ChatMessage::nonce).
                        nonce: None,
                    };
                    s.add_message(msg.clone());
                    if from != s.my_username {
                        let label = self.t(i18n::VOUS_ENVOIE_UN_FICHIER);
                        self.last_notification = Some(format!("{} {}", from, label));
                        self.notification_time = std::time::Instant::now();
                        self.has_unread = true;
                        if self.window_hidden {
                            Self::notify_native(from.clone(), label.to_string());
                        } else if self.enable_sound_notifications {
                            play_notification_sound();
                        }
                    }
                }
                AppEvent::MediaProgressed(progress) => {
                    let id = progress.id.clone();
                    if progress.failed {
                        self.media.progress.remove(&id);
                        self.media.presence.remove(&id);
                        if !progress.outgoing {
                            // Une réception interrompue n'a aucun fichier utile.
                            self.media.textures.remove(&id);
                            s.remove_media_message(&id);
                        }
                        self.last_notification =
                            Some(self.t(i18n::TRANSFERT_MEDIA_INTERROMPU).to_string());
                        self.notification_time = std::time::Instant::now();
                    } else if progress.finished {
                        self.media.progress.remove(&id);
                        // Le fichier est complet : recharger une éventuelle
                        // vignette, et reconnaître qu'il est désormais là.
                        self.media.textures.remove(&id);
                        self.media.presence.remove(&id);
                    } else {
                        self.media.progress.insert(id, progress);
                    }
                }
                AppEvent::MediaDeclined { peer, header } => {
                    // Un refus ne supprime pas la source : d'autres
                    // destinataires du groupe peuvent encore la recevoir.
                    self.media.progress.remove(&header.media.id);
                    s.add_message(super::media::refused_media_message(
                        &header.from,
                        &header.media.filename,
                        header.to_user.clone(),
                    ));
                    self.last_notification =
                        Some(format!("{} : {}", peer, self.t(i18n::FICHIER_REFUSE)));
                    self.notification_time = std::time::Instant::now();
                }
            }
        }
        s.clear_typing_if_old();
        self.typing_active = !s.typing_users.is_empty();
        drop(s);

        // Verrou relâché : c'est seulement ici qu'on émet.
        for request in outbound {
            self.net.try_send(request);
        }
        for (hash, request) in outbox_flush {
            // `try_send` ne dit que l'admission dans le canal : ni l'écriture
            // sur la socket, ni la réception. Le message reste donc dans la
            // file durable jusqu'à son ACK (cf. `mark_message_acked`), sans
            // quoi un arrêt au mauvais moment le laisserait en mémoire seule.
            if self.net.try_send(request.clone()) {
                self.state.lock_safe().mark_message_sent(hash, request);
            }
        }
    }

    /// Récupère les offres de médias volumineux (au-delà du seuil d'accord) en attente d'accord et
    /// les ajoute au bandeau d'acceptation.
    pub(crate) fn process_media_offers(&mut self) {
        while let Ok(offer) = self.media.offer_rx.try_recv() {
            if let Some(group_name) = offer.to_user.as_deref().and_then(|to| to.strip_prefix('#')) {
                let authorized =
                    group_media_authorized(&self.state.lock_safe(), group_name, &offer.from);
                if !authorized {
                    tracing::warn!(
                        "média de groupe refusé : {} n'est pas membre de #{}",
                        offer.from,
                        group_name
                    );
                    let _ = offer.decision_tx.send(false);
                    continue;
                }
                if !media_requires_ack(offer.size_bytes) {
                    let _ = offer.decision_tx.send(true);
                    continue;
                }
            }
            let label = self.t(i18n::VOUS_ENVOIE_UN_FICHIER);
            self.last_notification = Some(format!("{} {}", offer.from, label));
            self.notification_time = std::time::Instant::now();
            self.has_unread = true;
            if self.window_hidden {
                Self::notify_native(offer.from.clone(), label.to_string());
            } else if self.enable_sound_notifications {
                play_notification_sound();
            }
            self.media.pending_offers.push(offer);
        }
    }

    /// Tâches périodiques : retry ACK et écriture débouncée de la
    /// persistance (hors thread UI). La présence des pairs n'est plus
    /// vérifiée ici : la tâche discovery est autoritaire et émet
    /// `PeerDisconnected` (l'UI n'est réveillée que sur changement).
    pub(crate) fn periodic_tasks(&mut self) {
        if self.last_retry_time.elapsed().as_secs_f32() >= 2.0 {
            self.last_retry_time = std::time::Instant::now();
            let (retry_messages, failed) = self.state.lock_safe().get_retry_messages();
            for (hash, request) in retry_messages {
                let addr = request.to_addr;
                match self.net.send_tx.try_send(request.into()) {
                    Ok(()) => {
                        self.state.lock_safe().mark_retry_enqueued(hash);
                        tracing::debug!("retry message delivery vers {}", addr);
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        tracing::debug!("retry différé, file d'envoi pleine");
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        tracing::warn!("retry impossible, canal réseau fermé");
                    }
                }
            }
            if !failed.is_empty() {
                self.last_notification =
                    Some(self.t(i18n::ECHEC_DE_LIVRAISON_D_UN_MESSAGE).to_string());
                self.notification_time = std::time::Instant::now();
            }
        }
    }
}

fn group_media_authorized(state: &AppState, group_name: &str, sender: &str) -> bool {
    state.get_group(group_name).is_some_and(|group| {
        group.members.contains(&state.my_username)
            && group.members.iter().any(|member| member == sender)
    })
}

fn apply_group_event(s: &mut AppState, peer: &str, event: GroupEvent) {
    match event.action {
        GroupAction::Create { mut group } => {
            if group.owner != peer || !group.members.contains(&s.my_username) {
                return;
            }
            // Un pair antérieur aux identifiants n'en envoie pas : on le dérive
            // des champs immuables, à l'identique chez tous les membres.
            group.ensure_id();
            if let Some(existing) = s.groups.iter_mut().find(|g| g.id == group.id) {
                if existing.owner != peer {
                    return;
                }
                *existing = group;
            } else {
                s.groups.push(group);
            }
            s.save_groups();
        }
        GroupAction::AddMember { group_id, username } => {
            let Some(g) = s
                .groups
                .iter_mut()
                .find(|g| g.id == group_id && g.owner == peer)
            else {
                return;
            };
            if !g.members.contains(&username) {
                g.members.push(username);
                s.save_groups();
            }
        }
        GroupAction::RemoveMember { group_id, username } => {
            let allowed = s
                .groups
                .iter()
                .find(|g| g.id == group_id)
                .is_some_and(|g| g.owner == peer || peer == username);
            if allowed {
                s.apply_member_removal(&group_id, &username);
            }
        }
        GroupAction::Rename { group_id, new_name } => {
            let allowed = s.groups.iter().any(|g| g.id == group_id && g.owner == peer);
            if allowed {
                s.apply_group_rename(&group_id, new_name);
            }
        }
        GroupAction::Delete { group_id } => {
            let allowed = s.groups.iter().any(|g| g.id == group_id && g.owner == peer);
            if allowed {
                s.apply_group_delete(&group_id);
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/test_ui_events.rs"]
mod tests;
