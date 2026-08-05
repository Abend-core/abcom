//! Sélecteur de contenu Klipy façon Discord : GIF, Mèmes et Stickers.
//!
//! Trois onglets indépendants (GIF | Mèmes | Stickers) avec barre de
//! recherche partagée, grille masonry 2 colonnes et pagination infinie.
//!
//! Attribution obligatoire ToS Klipy : logo « Powered by KLIPY » en pied
//! de fenêtre, adapté au thème clair/sombre.

use eframe::egui;

use crate::app::AppState;
use crate::klipy::{GifFeed, GifItem, GifStatus};
use crate::message::{ChatMessage, MediaAttachment, MediaKind, SendRequest};
use crate::util::MutexExt;

use super::{AbcomApp, GifPickerTab};

const ATTR_DARK: &[u8] = include_bytes!("../../assets/klipy/attribution_dark_bg.png");
const ATTR_LIGHT: &[u8] = include_bytes!("../../assets/klipy/attribution_light_bg.png");

fn send_gif(app: &mut AbcomApp, gif: &GifItem) {
    let (my_name, selected_peer_name, selected_addr, all_peers) = {
        let s = app.state.lock_safe();
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
        reply_to: None,
        nonce: Some(ChatMessage::fresh_nonce()),
    };
    {
        let msg_hash = AppState::message_hash(&msg);
        let mut s = app.state.lock_safe();
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
        let _ = app.net.send_tx.try_send(SendRequest {
            to_addr: addr,
            message: msg,
        });
    } else {
        for peer in all_peers
            .iter()
            .filter(|p| p.online && !p.addr.ip().is_unspecified())
        {
            let _ = app.net.send_tx.try_send(SendRequest {
                to_addr: peer.addr,
                message: msg.clone(),
            });
        }
    }
}

/// Affiche la grille masonry pour un feed ; retourne (item choisi, besoin load_more).
fn show_feed_grid(
    ui: &mut egui::Ui,
    feed: &GifFeed,
    loading_label: &str,
    empty_label: &str,
    error_label: &str,
) -> (Option<GifItem>, bool) {
    let (items, status, has_next) = {
        let st = feed.lock();
        (st.items.clone(), st.status.clone(), st.has_next)
    };

    let mut chosen: Option<GifItem> = None;
    let mut want_more = false;

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
                        ui.colored_label(egui::Color32::from_rgb(220, 110, 110), error_label);
                    }
                    _ => {
                        ui.weak(empty_label);
                    }
                });
                return;
            }

            let col_w = (ui.available_width() - 6.0) / 2.0;
            ui.columns(2, |cols| {
                for (i, item) in items.iter().enumerate() {
                    let col = &mut cols[i % 2];
                    let size =
                        super::media::gif_display_size(item.width, item.height, col_w, col_w * 2.0);
                    // Gel hors écran : seuls les aperçus visibles dans la
                    // grille sont décodés/animés (la place reste réservée).
                    let (rect, resp) = col.allocate_exact_size(size, egui::Sense::click());
                    if col.is_rect_visible(rect) {
                        col.put(
                            rect,
                            egui::Image::from_uri(item.preview_url.clone())
                                .fit_to_exact_size(size)
                                .corner_radius(6.0),
                        );
                    }
                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() {
                        chosen = Some(item.clone());
                    }
                    col.add_space(6.0);
                }
            });

            let (sentinel, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
            if has_next && ui.is_rect_visible(sentinel) {
                want_more = true;
            }
            if status == GifStatus::Loading {
                ui.add_space(6.0);
                ui.vertical_centered(|ui| ui.spinner());
            }
        });

    (chosen, want_more)
}

/// Vérifie si un feed doit être initialisé (pas encore chargé).
fn needs_init(feed: &GifFeed) -> bool {
    let st = feed.lock();
    st.status == GifStatus::Idle && st.items.is_empty()
}

impl AbcomApp {
    /// Libère du cache d'images egui les aperçus des trois feeds Klipy
    /// (appelé à la fermeture du picker : les frames WebP décodées des
    /// aperçus représentent plusieurs dizaines de Mo par page).
    pub(crate) fn forget_gif_previews(&self, ctx: &egui::Context) {
        for feed in [&self.gif_feed, &self.meme_feed, &self.sticker_feed] {
            for item in feed.lock().items.iter() {
                ctx.forget_image(&item.preview_url);
            }
        }
    }

    pub(crate) fn show_gif_picker_window(&mut self, ctx: &egui::Context, gif_button_clicked: bool) {
        if !self.show_gif_picker {
            if self.gif_picker_was_open {
                self.gif_picker_was_open = false;
                self.forget_gif_previews(ctx);
            }
            return;
        }
        self.gif_picker_was_open = true;
        let Some(key) = crate::config::klipy_api_key() else {
            self.show_gif_picker = false;
            return;
        };
        let locale = match self.ui_language {
            super::UiLanguage::French => "fr",
            super::UiLanguage::English => "en",
        };

        // Charge les tendances de l'onglet GIF dès l'ouverture.
        if needs_init(&self.gif_feed) {
            self.gif_feed.load_trending(ctx, &key, locale);
        }

        let tab_gif_label = "GIF";
        let tab_meme_label = self.tr("Mèmes", "Memes");
        let tab_sticker_label = "Stickers";
        let search_hint = self.tr("Search KLIPY", "Search KLIPY");
        let loading_label = self.tr("Chargement…", "Loading…");
        let empty_label = self.tr("Aucun résultat", "No results");
        let error_label = self.tr("Erreur de chargement", "Loading error");

        let mut picker_rect: Option<egui::Rect> = None;
        let mut chosen: Option<GifItem> = None;
        let mut want_load_more = false;

        let window = egui::Window::new(self.tr("GIF & Stickers", "GIF & Stickers"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-8.0, -60.0))
            .resizable(false)
            .collapsible(false)
            .fixed_size([360.0, 460.0]);

        if let Some(resp) = window.show(ctx, |ui| {
            // ── Onglets centrés ──────────────────────────────────────────────
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let tab = self.gif_picker_tab;

                    if ui
                        .selectable_label(tab == GifPickerTab::Gif, tab_gif_label)
                        .clicked()
                        && tab != GifPickerTab::Gif
                    {
                        self.gif_picker_tab = GifPickerTab::Gif;
                        if needs_init(&self.gif_feed) {
                            self.gif_feed.load_trending(ctx, &key, locale);
                        }
                    }
                    if ui
                        .selectable_label(tab == GifPickerTab::Meme, tab_meme_label)
                        .clicked()
                        && tab != GifPickerTab::Meme
                    {
                        self.gif_picker_tab = GifPickerTab::Meme;
                        if needs_init(&self.meme_feed) {
                            self.meme_feed.load_trending(ctx, &key, locale);
                        }
                    }
                    if ui
                        .selectable_label(tab == GifPickerTab::Sticker, tab_sticker_label)
                        .clicked()
                        && tab != GifPickerTab::Sticker
                    {
                        self.gif_picker_tab = GifPickerTab::Sticker;
                        if needs_init(&self.sticker_feed) {
                            self.sticker_feed.load_trending(ctx, &key, locale);
                        }
                    }
                });
            });
            ui.add_space(2.0);

            // ── Barre de recherche ───────────────────────────────────────────
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

            // ── Grille de l'onglet actif ─────────────────────────────────────
            let (c, w) = match self.gif_picker_tab {
                GifPickerTab::Gif => {
                    show_feed_grid(ui, &self.gif_feed, loading_label, empty_label, error_label)
                }
                GifPickerTab::Meme => {
                    show_feed_grid(ui, &self.meme_feed, loading_label, empty_label, error_label)
                }
                GifPickerTab::Sticker => show_feed_grid(
                    ui,
                    &self.sticker_feed,
                    loading_label,
                    empty_label,
                    error_label,
                ),
            };
            chosen = c;
            want_load_more = w;

            // ── Attribution Klipy (ToS obligatoire) ──────────────────────────
            ui.separator();
            let dark = ui.visuals().dark_mode;
            let (bytes, uri) = if dark {
                (ATTR_DARK, "bytes://klipy_attr_dark")
            } else {
                (ATTR_LIGHT, "bytes://klipy_attr_light")
            };
            ui.add_space(2.0);
            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Image::from_bytes(uri, bytes).fit_to_exact_size(egui::vec2(140.0, 30.0)),
                );
            });
            ui.add_space(2.0);
        }) {
            picker_rect = Some(resp.response.rect);
        }

        // ── Debounce recherche (300 ms) sur l'onglet actif ───────────────────
        let pending = self.gif_query.trim().to_string();
        let feed_query = match self.gif_picker_tab {
            GifPickerTab::Gif => self.gif_feed.lock().query.clone(),
            GifPickerTab::Meme => self.meme_feed.lock().query.clone(),
            GifPickerTab::Sticker => self.sticker_feed.lock().query.clone(),
        };
        if pending != feed_query {
            if self.gif_last_input.elapsed() >= std::time::Duration::from_millis(300) {
                match self.gif_picker_tab {
                    GifPickerTab::Gif => self.gif_feed.search(ctx, &key, locale, &pending),
                    GifPickerTab::Meme => self.meme_feed.search(ctx, &key, locale, &pending),
                    GifPickerTab::Sticker => self.sticker_feed.search(ctx, &key, locale, &pending),
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
        }

        if want_load_more {
            match self.gif_picker_tab {
                GifPickerTab::Gif => self.gif_feed.load_more(ctx, &key, locale),
                GifPickerTab::Meme => self.meme_feed.load_more(ctx, &key, locale),
                GifPickerTab::Sticker => self.sticker_feed.load_more(ctx, &key, locale),
            }
        }

        if let Some(gif) = chosen {
            send_gif(self, &gif);
            self.show_gif_picker = false;
        }

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
