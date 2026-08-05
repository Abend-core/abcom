use eframe::egui;

use super::{sound::play_notification_sound, AbcomApp};
use crate::app::AppState;
use crate::message::{
    AppEvent, AvatarRequest, ChatMessage, GroupAction, GroupEvent, MessageAck, MessageAckRequest,
    ReadReceipt, ReadReceiptRequest, SendGroupRequest,
};
use crate::util::MutexExt;

impl AbcomApp {
    /// Textures emoji : les PNG sont décodés dans un thread au démarrage
    /// (cf. `spawn_emoji_decoder`) ; ici on ne fait que récupérer le résultat
    /// et créer les textures (rapide). Tant qu'il n'est pas prêt, l'UI
    /// s'affiche sans emojis et repeint brièvement en attendant.
    pub(crate) fn lazy_load_emoji(&mut self, ctx: &egui::Context) {
        if self.emoji_textures_loaded {
            return;
        }
        let Some(rx) = &self.emoji_decode_rx else {
            return;
        };
        let images = match rx.try_recv() {
            Ok(images) => images,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // Décodage en cours : re-tenter très bientôt.
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
                return;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.emoji_decode_rx = None;
                return;
            }
        };
        self.emoji_decode_rx = None;

        self.emoji_textures = images
            .into_iter()
            .map(|(ch, color_image)| {
                let texture = ctx.load_texture(
                    format!("emoji_{ch}"),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                (ch, texture)
            })
            .collect();

        self.emoji_map = self
            .emoji_textures
            .iter()
            .enumerate()
            .map(|(i, (ch, _))| (ch.clone(), i))
            .collect();
        let available: Vec<String> = self
            .emoji_textures
            .iter()
            .map(|(ch, _)| ch.clone())
            .collect();

        let (alias_to_char, aliases) = super::emoji_picker::build_emoji_shortcode_index(&available);
        self.emoji_alias_to_char = alias_to_char;
        self.emoji_aliases = aliases;
        self.emoji_textures_loaded = true;

        // Les messages parsés avant l'arrivée du registre ont une détection
        // « emoji seul » erronée : on reconstruit le cache du fil.
        self.chat_cache.invalidate();
    }

    /// Dépile les événements réseau reçus depuis les tâches tokio
    pub(crate) fn process_events(&mut self) {
        let mut s = self.state.lock_safe();
        while let Ok(evt) = self.event_rx.try_recv() {
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
                            .map(|g| s.is_in_group(g))
                            .unwrap_or(false);
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
                            for addr in recipients {
                                ack_reqs.push(MessageAckRequest {
                                    to_addr: addr,
                                    ack: MessageAck {
                                        from: s.my_username.clone(),
                                        to: msg.from.clone(),
                                        message_hash: msg_hash,
                                        timestamp: now.clone(),
                                    },
                                });
                                if already_reading {
                                    receipt_reqs.push(ReadReceiptRequest {
                                        to_addr: addr,
                                        receipt: ReadReceipt {
                                            from: s.my_username.clone(),
                                            to: msg.from.clone(),
                                            message_hash: msg_hash,
                                            timestamp: now.clone(),
                                        },
                                    });
                                }
                            }

                            drop(s);
                            for req in ack_reqs {
                                let _ = self.send_ack_tx.try_send(req);
                            }
                            for req in receipt_reqs {
                                let _ = self.send_read_receipt_tx.try_send(req);
                            }
                            s = self.state.lock_safe();
                        }
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
                        let _ = self.send_group_tx.try_send(SendGroupRequest {
                            to_addr: addr,
                            event,
                        });
                    }
                    // Première découverte (depuis le dernier envoi) : on partage
                    // notre avatar pour qu'il s'affiche chez ce pair.
                    if !self.avatar_sent_to.contains(&username) {
                        if let Some(announce) = s.avatar_announce() {
                            let request = AvatarRequest {
                                to_addr: addr,
                                announce,
                            };
                            if self.send_avatar_tx.try_send(request).is_ok() {
                                self.avatar_sent_to.insert(username);
                            }
                        }
                    }
                }
                AppEvent::PeerDisconnected { username } => {
                    if let Some(peer) = s.peers.iter_mut().find(|p| p.username == username) {
                        peer.online = false;
                    }
                    // Réémettre l'avatar à la prochaine reconnexion de ce pair.
                    self.avatar_sent_to.remove(&username);
                }
                AppEvent::UserTyping(username) => s.set_user_typing(username),
                AppEvent::GroupEventReceived(evt) => match evt.action {
                    GroupAction::Create { group } => {
                        // Création ou re-synchronisation par le propriétaire :
                        // l'état reçu remplace le nôtre. Un groupe dont nous ne
                        // sommes pas membre ne nous concerne pas.
                        if group.members.contains(&s.my_username) {
                            if let Some(existing) =
                                s.groups.iter_mut().find(|g| g.name == group.name)
                            {
                                *existing = group;
                            } else {
                                s.groups.push(group);
                            }
                            s.save_groups();
                        }
                    }
                    GroupAction::AddMember {
                        group_name,
                        username,
                    } => {
                        if let Some(g) = s.groups.iter_mut().find(|g| g.name == group_name) {
                            if !g.members.contains(&username) {
                                g.members.push(username);
                                s.save_groups();
                            }
                        }
                    }
                    GroupAction::RemoveMember {
                        group_name,
                        username,
                    } => {
                        // Départ volontaire ou exclusion — même règle de
                        // succession que localement ; si c'est nous, le salon
                        // et son historique local disparaissent.
                        s.apply_member_removal(&group_name, &username);
                    }
                    GroupAction::Rename {
                        group_name,
                        new_name,
                    } => {
                        // Valide et migre l'historique vers la nouvelle clé.
                        s.apply_group_rename(&group_name, new_name);
                    }
                    GroupAction::Delete { group_name } => {
                        s.apply_group_delete(&group_name);
                    }
                },
                AppEvent::ReadReceiptReceived(receipt) => {
                    s.mark_message_read(receipt.message_hash, receipt.from.clone());
                }
                AppEvent::MessageAckReceived(ack) => {
                    s.mark_message_acked(ack.message_hash);
                    // Détail nominatif « reçu par » (popup « … » des salons).
                    s.mark_message_delivered_by(ack.message_hash, ack.from.clone());
                }
                AppEvent::ReactionReceived(event) => {
                    s.apply_reaction_event(&event);
                }
                AppEvent::KeyChanged { username } => {
                    // Alerte sécurité : la clé du pair ne correspond plus à
                    // celle épinglée — la connexion a été refusée.
                    let label = self.tr(
                        "la clé d'identité a changé, connexion refusée",
                        "identity key changed, connection refused",
                    );
                    self.last_notification = Some(format!("⚠️ {} : {}", username, label));
                    self.notification_time = std::time::Instant::now();
                    if self.window_hidden {
                        Self::notify_native(format!("⚠️ {username}"), label.to_string());
                    } else if self.enable_sound_notifications {
                        play_notification_sound();
                    }
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
                        let label = self.tr("vous envoie un fichier", "is sending you a file");
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
                        // Refus ou erreur : on retire la carte, le message et le
                        // fichier (côté émetteur comme destinataire).
                        self.media_progress.remove(&id);
                        self.media_textures.remove(&id);
                        s.remove_media_message(&id);
                        self.last_notification = Some(
                            self.tr("Transfert média interrompu", "Media transfer interrupted")
                                .to_string(),
                        );
                        self.notification_time = std::time::Instant::now();
                    } else if progress.finished {
                        self.media_progress.remove(&id);
                        // Le fichier est complet : recharger une éventuelle vignette.
                        self.media_textures.remove(&id);
                    } else {
                        self.media_progress.insert(id, progress);
                    }
                }
                AppEvent::MediaDeclined(header) => {
                    // Côté émetteur : on retire la carte « en attente » et on
                    // annote le fil que le fichier a été refusé.
                    self.media_progress.remove(&header.media.id);
                    self.media_textures.remove(&header.media.id);
                    s.remove_media_message(&header.media.id);
                    s.add_message(super::media::refused_media_message(
                        &header.from,
                        &header.media.filename,
                        header.to_user.clone(),
                    ));
                }
            }
        }
        s.clear_typing_if_old();
        self.typing_active = !s.typing_users.is_empty();
    }

    /// Récupère les offres de médias volumineux (> 1 Go) en attente d'accord et
    /// les ajoute au bandeau d'acceptation.
    pub(crate) fn process_media_offers(&mut self) {
        while let Ok(offer) = self.media_offer_rx.try_recv() {
            let label = self.tr("vous envoie un fichier", "is sending you a file");
            self.last_notification = Some(format!("{} {}", offer.from, label));
            self.notification_time = std::time::Instant::now();
            self.has_unread = true;
            if self.window_hidden {
                Self::notify_native(offer.from.clone(), label.to_string());
            } else if self.enable_sound_notifications {
                play_notification_sound();
            }
            self.pending_media_offers.push(offer);
        }
    }

    /// Tâches périodiques : retry ACK et écriture débouncée de la
    /// persistance (hors thread UI). La présence des pairs n'est plus
    /// vérifiée ici : la tâche discovery est autoritaire et émet
    /// `PeerDisconnected` (l'UI n'est réveillée que sur changement).
    pub(crate) fn periodic_tasks(&mut self) {
        if self.last_retry_time.elapsed().as_secs_f32() >= 2.0 {
            self.last_retry_time = std::time::Instant::now();
            let retry_messages = self.state.lock_safe().get_retry_messages();
            for (_hash, addr) in retry_messages {
                tracing::debug!("retry message delivery vers {}", addr);
            }
        }
    }
}
