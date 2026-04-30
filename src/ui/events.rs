use eframe::egui;

use crate::app::AppState;
use crate::message::{AppEvent, GroupAction, MessageAck, MessageAckRequest};
use crate::transfer::TransferStatus;

use super::{sound::play_notification_sound, AbcomApp};

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
                    // ACK automatique pour les messages privés
                    if msg.to_user.is_some() && msg.from != s.my_username {
                        if let Some(peer) = s.peers.iter().find(|p| p.username == msg.from) {
                            let msg_hash = AppState::message_hash(&msg);
                            let ack = MessageAck {
                                from: s.my_username.clone(),
                                to: msg.from.clone(),
                                message_hash: msg_hash,
                                timestamp: chrono::Local::now().format("%H:%M").to_string(),
                            };
                            let req = MessageAckRequest {
                                to_addr: peer.addr,
                                ack,
                            };
                            drop(s);
                            let _ = self.send_ack_tx.try_send(req);
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
                AppEvent::PeerDiscovered { username, addr } => s.add_peer(username, addr),
                AppEvent::PeerDisconnected { username } => {
                    if let Some(peer) = s.peers.iter_mut().find(|p| p.username == username) {
                        peer.online = false;
                    }
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
                AppEvent::TransferUpdated(progress) => {
                    let transfer_id = progress.transfer_id.clone();
                    let status = progress.status.clone();
                    let label = progress.label.clone();
                    let peer = progress.peer.clone();
                    let detail = progress.detail.clone();
                    drop(s);

                    self.transfer_progress.insert(transfer_id, progress);

                    match status {
                        TransferStatus::Completed => {
                            self.last_notification = Some(format!(
                                "Transfer complete: {} ({})",
                                label, peer
                            ));
                            if !detail.is_empty() {
                                self.last_notification = Some(format!(
                                    "Transfer complete: {} ({}) -> {}",
                                    label, peer, detail
                                ));
                            }
                            self.notification_time = std::time::Instant::now();
                        }
                        TransferStatus::Failed => {
                            self.last_notification = Some(format!(
                                "Transfer failed: {} ({})",
                                label, peer
                            ));
                            self.notification_time = std::time::Instant::now();
                        }
                        _ => {}
                    }

                    s = self.state.lock().unwrap();
                }
            }
        }
        s.clear_typing_if_old();
    }

    /// Tâches périodiques : nettoyage pairs, détection réseau, retry ACK
    pub(crate) fn periodic_tasks(&mut self) {
        if self.last_cleanup_time.elapsed().as_secs() >= 5 {
            self.last_cleanup_time = std::time::Instant::now();
            {
                let mut s = self.state.lock().unwrap();
                s.cleanup_inactive_peers(10);
            }
            if self.last_network_check.elapsed().as_secs() >= 15 {
                self.last_network_check = std::time::Instant::now();
                let (new_id, new_subnet) = crate::app::AppState::detect_network_id();
                let (old_id, old_subnet) = {
                    let s = self.state.lock().unwrap();
                    (s.current_network_id.clone(), s.current_subnet.clone())
                };
                if new_id != old_id || new_subnet != old_subnet {
                    let mut s = self.state.lock().unwrap();
                    s.current_network_id = new_id.clone();
                    s.current_subnet = new_subnet.clone();
                    if let Some(ref id) = new_id {
                        s.ensure_network_known(id, new_subnet.as_deref());
                    }
                    drop(s);
                    self.selected_network_filter = new_id;
                }
            }
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
