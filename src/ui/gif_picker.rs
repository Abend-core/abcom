//! Sélecteur de GIF façon Discord : barre de recherche + grille masonry de
//! vignettes animées (Klipy), insertion du GIF choisi dans la conversation.

use eframe::egui;

use crate::app::AppState;
use crate::klipy::{GifItem, GifStatus};
use crate::message::{ChatMessage, MediaAttachment, MediaKind, SendRequest};

use super::AbcomApp;

/// Construit et envoie le message GIF (URL seule) vers la conversation courante,
/// en réutilisant le même ciblage destinataire/diffusion que les messages texte.
fn send_gif(app: &mut AbcomApp, gif: &GifItem) {
    let (my_name, selected_peer_name, selected_addr, all_peers) = {
        let s = app.state.lock().unwrap();
        (
            s.my_username.clone(),
            s.selected_conversation.clone(),
            s.selected_peer_addr(),
            s.peers.clone(),
        )
    };

    let media = MediaAttachment {
        id: gif.id.clone(),
        filename: "gif.webp".to_string(),
        kind: MediaKind::Gif,
        size_bytes: 0,
        url: Some(gif.full_url.clone()),
        width: gif.width,
        height: gif.height,
    };
    let now = chrono::Local::now();
    let msg = ChatMessage {
        from: my_name,
        content: String::new(),
        timestamp: now.format("%H:%M").to_string(),
        timestamp_epoch: Some(now.timestamp() as u64),
        to_user: selected_peer_name.clone(),
        media: Some(media),
    };

    {
        let msg_hash = AppState::message_hash(&msg);
        let mut s = app.state.lock().unwrap();
        s.add_message(msg.clone());
        if let Some(peer_name) = &selected_peer_name {
            if !peer_name.starts_with('#') {
                let peer_addr = s
                    .peers
                    .iter()
                    .find(|p| p.username == *peer_name)
                    .map(|p| p.addr);
                if let Some(addr) = peer_addr {
                    s.mark_message_sent(msg_hash, addr);
                }
            }
        }
    }

    if let Some(addr) = selected_addr {
        let _ = app.send_tx.try_send(SendRequest {
            to_addr: addr,
            message: msg,
        });
    } else {
        for peer in all_peers
            .iter()
            .filter(|p| p.online && !p.addr.ip().is_unspecified())
        {
            let _ = app.send_tx.try_send(SendRequest {
                to_addr: peer.addr,
                message: msg.clone(),
            });
        }
    }
}

impl AbcomApp {
    /// Affiche la fenêtre du sélecteur de GIF (si ouvert) et gère recherche,
    /// pagination, sélection et fermeture au clic extérieur.
    pub(crate) fn show_gif_picker_window(&mut self, ctx: &egui::Context, gif_button_clicked: bool) {
        if !self.show_gif_picker {
            return;
        }
        // Sans clé API, le sélecteur ne peut rien charger : on referme.
        let Some(key) = crate::config::klipy_api_key() else {
            self.show_gif_picker = false;
            return;
        };
        let locale = match self.ui_language {
            super::UiLanguage::French => "fr",
            super::UiLanguage::English => "en",
        };

        // Premier affichage : charge les tendances.
        let needs_initial = {
            let st = self.gif_feed.lock();
            st.status == GifStatus::Idle && st.items.is_empty()
        };
        if needs_initial {
            self.gif_feed.load_trending(ctx, &key, locale);
        }

        let search_hint = self.tr("Rechercher un GIF", "Search for a GIF");
        let loading_label = self.tr("Chargement…", "Loading…");
        let empty_label = self.tr("Aucun résultat", "No results");
        let error_label = self.tr("Erreur de chargement", "Loading error");

        let mut picker_rect: Option<egui::Rect> = None;
        let mut chosen: Option<GifItem> = None;
        let mut want_load_more = false;

        let window = egui::Window::new(self.tr("GIF", "GIF"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-8.0, -60.0))
            .resizable(false)
            .collapsible(false)
            .fixed_size([360.0, 420.0]);

        if let Some(resp) = window.show(ctx, |ui| {
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.gif_query)
                    .hint_text(search_hint)
                    .desired_width(f32::INFINITY),
            );
            if gif_button_clicked {
                edit.request_focus();
            }
            if edit.changed() {
                self.gif_last_input = std::time::Instant::now();
            }
            ui.separator();

            // Instantané de l'état partagé pour le rendu de cette frame.
            let (items, status, has_next) = {
                let st = self.gif_feed.lock();
                (st.items.clone(), st.status.clone(), st.has_next)
            };

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if items.is_empty() {
                        ui.add_space(16.0);
                        ui.vertical_centered(|ui| match &status {
                            GifStatus::Loading => {
                                ui.spinner();
                                ui.label(loading_label);
                            }
                            GifStatus::Error(_) => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(220, 110, 110),
                                    error_label,
                                );
                            }
                            _ => {
                                ui.weak(empty_label);
                            }
                        });
                        return;
                    }

                    // Grille masonry sur 2 colonnes (façon Discord) : on répartit
                    // les vignettes en alternance, chacune calée sur la largeur de
                    // colonne en gardant son ratio.
                    let col_w = (ui.available_width() - 6.0) / 2.0;
                    ui.columns(2, |cols| {
                        for (i, item) in items.iter().enumerate() {
                            let col = &mut cols[i % 2];
                            // Taille forcée d'après le ratio : vignettes nettes,
                            // uniformes en largeur, hauteur plafonnée pour les
                            // GIF très allongés.
                            let size = super::media::gif_display_size(
                                item.width,
                                item.height,
                                col_w,
                                col_w * 2.0,
                            );
                            let resp = col
                                .add(
                                    egui::Image::from_uri(item.preview_url.clone())
                                        .fit_to_exact_size(size)
                                        .corner_radius(6.0)
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if resp.clicked() {
                                chosen = Some(item.clone());
                            }
                            col.add_space(6.0);
                        }
                    });

                    // Pagination infinie : sentinelle en bas de liste.
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 1.0),
                        egui::Sense::hover(),
                    );
                    if has_next && ui.is_rect_visible(rect) {
                        want_load_more = true;
                    }
                    if status == GifStatus::Loading {
                        ui.add_space(6.0);
                        ui.vertical_centered(|ui| ui.spinner());
                    }
                });
        }) {
            picker_rect = Some(resp.response.rect);
        }

        // Recherche anti-rebond : ~300 ms après la dernière frappe, si le texte
        // diffère de la requête déjà chargée.
        let pending = self.gif_query.trim().to_string();
        let feed_query = self.gif_feed.lock().query.clone();
        if pending != feed_query {
            if self.gif_last_input.elapsed() >= std::time::Duration::from_millis(300) {
                self.gif_feed.search(ctx, &key, locale, &pending);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
        }

        if want_load_more {
            self.gif_feed.load_more(ctx, &key, locale);
        }

        if let Some(gif) = chosen {
            send_gif(self, &gif);
            self.show_gif_picker = false;
        }

        // Fermeture au clic en dehors de la fenêtre (hors clic sur le bouton GIF).
        if !gif_button_clicked && ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if let Some(rect) = picker_rect {
                    if !rect.contains(pos) {
                        self.show_gif_picker = false;
                    }
                }
            }
        }
    }
}
