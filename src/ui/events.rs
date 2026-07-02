use eframe::egui;

use super::{sound::play_notification_sound, AbcomApp};
use crate::app::AppState;
use crate::message::{
    AppEvent, AvatarRequest, ChatMessage, GroupAction, MessageAck, MessageAckRequest, ReadReceipt,
    ReadReceiptRequest,
};

impl AbcomApp {
    /// Chargement paresseux des textures emoji (nécessite le contexte egui)
    pub(crate) fn lazy_load_emoji(&mut self, ctx: &egui::Context) {
        if self.emoji_textures_loaded {
            return;
        }
        self.emoji_textures = crate::emoji_registry::EMOJI_DATA
            .iter()
            .filter_map(|(ch, bytes)| {
                image::load_from_memory(bytes).ok().map(|img| {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        rgba.as_raw(),
                    );
                    let texture = ctx.load_texture(
                        format!("emoji_{ch}"),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    (ch.to_string(), texture)
                })
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
    }

    /// Dépile les événements réseau reçus depuis les tâches tokio
    pub(crate) fn process_events(&mut self) {
        let mut s = self.state.lock().unwrap();
        while let Ok(evt) = self.event_rx.try_recv() {
            match evt {
                AppEvent::MessageReceived(msg) => {
                    // ACK automatique (livraison) pour les messages privés.
                    // Le ReadReceipt (lecture) n'est envoyé que si la conversation
                    // est déjà ouverte et la fenêtre active — sinon différé à l'ouverture.
                    if msg.to_user.is_some() && msg.from != s.my_username {
                        if let Some(peer) = s.peers.iter().find(|p| p.username == msg.from) {
                            let msg_hash = AppState::message_hash(&msg);
                            let ack = MessageAck {
                                from: s.my_username.clone(),
                                to: msg.from.clone(),
                                message_hash: msg_hash,
                                timestamp: chrono::Local::now().format("%H:%M").to_string(),
                            };
                            let ack_req = MessageAckRequest {
                                to_addr: peer.addr,
                                ack,
                            };

                            // ReadReceipt uniquement si la conv est déjà ouverte + fenêtre focalisée
                            let already_reading = self.window_focused
                                && s.selected_conversation == Some(msg.from.clone());
                            let receipt_req = already_reading.then(|| ReadReceiptRequest {
                                to_addr: peer.addr,
                                receipt: ReadReceipt {
                                    from: s.my_username.clone(),
                                    to: msg.from.clone(),
                                    message_hash: msg_hash,
                                    timestamp: chrono::Local::now().format("%H:%M").to_string(),
                                },
                            });

                            drop(s);
                            let _ = self.send_ack_tx.try_send(ack_req);
                            if let Some(rr) = receipt_req {
                                let _ = self.send_read_receipt_tx.try_send(rr);
                            }
                            s = self.state.lock().unwrap();
                        }
                    }

                    s.add_message(msg.clone());
                    if msg.from != s.my_username {
                        self.last_notification = Some(format!("{}: {}", msg.from, msg.content));
                        self.notification_time = std::time::Instant::now();
                        self.has_unread = true;
                        let source_conv: Option<String> = if msg.to_user.is_none() {
                            None
                        } else {
                            Some(msg.from.clone())
                        };
                        let already_in_conv = s.selected_conversation == source_conv;
                        let conv_muted = self.muted_conversations.contains(&source_conv);
                        if self.enable_sound_notifications && !already_in_conv && !conv_muted {
                            play_notification_sound();
                        }
                    }
                }
                AppEvent::PeerDiscovered { username, addr } => {
                    s.add_peer(username.clone(), addr);
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
                AppEvent::UserStoppedTyping(_) => s.clear_typing_if_old(),
                AppEvent::GroupEventReceived(evt) => match evt.action {
                    GroupAction::Create { group } => {
                        if !s.groups.iter().any(|g| g.name == group.name) {
                            s.groups.push(group);
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
                        if let Some(g) = s.groups.iter_mut().find(|g| g.name == group_name) {
                            g.members.retain(|m| m != &username);
                            s.save_groups();
                        }
                    }
                    GroupAction::Rename {
                        group_name,
                        new_name,
                    } => {
                        if let Some(g) = s.groups.iter_mut().find(|g| g.name == group_name) {
                            g.name = new_name;
                            s.save_groups();
                        }
                    }
                    GroupAction::Delete { group_name } => {
                        s.groups.retain(|g| g.name != group_name);
                        s.save_groups();
                    }
                },
                AppEvent::ReadReceiptReceived(receipt) => {
                    s.mark_message_read(receipt.message_hash, receipt.from.clone());
                }
                AppEvent::MessageAckReceived(ack) => {
                    s.mark_message_acked(ack.message_hash);
                }
                AppEvent::ReactionReceived(event) => {
                    s.apply_reaction_event(&event);
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
                    };
                    s.add_message(msg.clone());
                    if from != s.my_username {
                        self.last_notification = Some(format!(
                            "{} {}",
                            from,
                            self.tr("vous envoie un fichier", "is sending you a file")
                        ));
                        self.notification_time = std::time::Instant::now();
                        self.has_unread = true;
                        if self.enable_sound_notifications {
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
    }

    /// Récupère les offres de médias volumineux (> 1 Go) en attente d'accord et
    /// les ajoute au bandeau d'acceptation.
    pub(crate) fn process_media_offers(&mut self) {
        while let Ok(offer) = self.media_offer_rx.try_recv() {
            self.last_notification = Some(format!(
                "{} {}",
                offer.from,
                self.tr("vous envoie un fichier", "is sending you a file")
            ));
            self.notification_time = std::time::Instant::now();
            self.has_unread = true;
            if self.enable_sound_notifications {
                play_notification_sound();
            }
            self.pending_media_offers.push(offer);
        }
    }

    /// Tâches périodiques : nettoyage des pairs inactifs et retry ACK
    pub(crate) fn periodic_tasks(&mut self) {
        if self.last_cleanup_time.elapsed().as_secs() >= 5 {
            self.last_cleanup_time = std::time::Instant::now();
            let mut s = self.state.lock().unwrap();
            s.cleanup_inactive_peers(10);
        }

        if self.last_retry_time.elapsed().as_secs_f32() >= 2.0 {
            self.last_retry_time = std::time::Instant::now();
            let retry_messages = self.state.lock().unwrap().get_retry_messages();
            for (_hash, addr) in retry_messages {
                eprintln!("[ui] Retry message delivery vers {}", addr);
            }
        }
    }
}
