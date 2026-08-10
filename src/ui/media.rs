//! Affichage des médias dans le fil : vignette d'image cliquable, carte fichier
//! téléchargeable, visionneuse plein écran et téléchargement vers le dossier
//! Téléchargements du système.

use super::i18n;
use std::path::Path;

use eframe::egui;

use crate::message::{
    extension_lower, AppEvent, ChatMessage, MediaAttachment, MediaKind, MediaProgress,
};
use crate::util::MutexExt;

use super::chat_panel::format_bytes;
use super::AbcomApp;

/// Largeur maximale d'une vignette d'image dans le fil.
const THUMB_MAX_WIDTH: f32 = 320.0;
/// Hauteur maximale d'une vignette d'image dans le fil.
const THUMB_MAX_HEIGHT: f32 = 260.0;
/// Bornes d'affichage d'un GIF dans le fil (plus grand qu'une vignette image).
const GIF_MAX_WIDTH: f32 = 360.0;
const GIF_MAX_HEIGHT: f32 = 300.0;

/// Taille d'affichage d'un GIF/vignette à partir des dimensions fournies par
/// l'API, calée dans la boîte `max_w`×`max_h` en conservant le ratio. Autorise
/// l'agrandissement (les variantes WebP de Klipy sont parfois plus petites que
/// la boîte) ; à défaut de dimensions connues, remplit la boîte.
pub(crate) fn gif_display_size(
    width: Option<u32>,
    height: Option<u32>,
    max_w: f32,
    max_h: f32,
) -> egui::Vec2 {
    let w = width.unwrap_or(0) as f32;
    let h = height.unwrap_or(0) as f32;
    if w <= 0.0 || h <= 0.0 {
        return egui::vec2(max_w, max_h);
    }
    let scale = (max_w / w).min(max_h / h);
    egui::vec2(w * scale, h * scale)
}

/// Nom affiché/transmis pour un média : nom du fichier, ou `dossier.zip`.
pub(crate) fn media_display_name(path: &Path) -> String {
    if path.is_dir() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("dossier");
        format!("{name}.zip")
    } else {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("fichier")
            .to_string()
    }
}

/// Message « fichier refusé » ajouté au fil, attribué à l'expéditeur du média
/// (visible à l'identique chez l'émetteur et le destinataire).
pub(crate) fn refused_media_message(
    sender: &str,
    filename: &str,
    to_user: Option<String>,
) -> ChatMessage {
    let now = chrono::Local::now();
    ChatMessage {
        from: sender.to_string(),
        content: format!("Fichier refusé : {filename}"),
        timestamp: now.format("%H:%M").to_string(),
        timestamp_epoch: Some(now.timestamp() as u64),
        to_user,
        media: None,
        reply_to: None,
        // Pas de nonce : ce message est construit indépendamment chez
        // l'émetteur et le destinataire et doit avoir le même hash des deux côtés.
        nonce: None,
    }
}

/// Identifiant unique de média (sert de nom de fichier en cache, extension
/// conservée). Préfixé par un horodatage µs pour garantir l'unicité.
pub(crate) fn media_id(filename: &str) -> String {
    let micros = chrono::Utc::now().timestamp_micros();
    let safe: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{micros}-{safe}")
}

/// Action déclenchée par l'utilisateur sur un média du fil.
pub(crate) enum MediaAction {
    /// Ouvrir l'image en grand dans la visionneuse.
    View,
    /// Télécharger le fichier vers le dossier Téléchargements.
    Download,
}

/// Largeur d'affichage qui respecte les bornes `max_w`/`max_h` tout en gardant
/// le ratio de la texture (on ne contraint que la largeur, egui gère le reste).
fn fitted_width(texture: &egui::TextureHandle, max_w: f32, max_h: f32) -> f32 {
    let size = texture.size_vec2();
    if size.y <= 0.0 {
        return max_w;
    }
    let aspect = size.x / size.y;
    max_w.min(max_h * aspect).min(size.x)
}

/// Carte de transfert média en cours : nom, taille et barre de progression.
pub(crate) fn render_media_progress(
    ui: &mut egui::Ui,
    media: &MediaAttachment,
    progress: &MediaProgress,
) {
    let width = FILE_CARD_WIDTH.min(ui.available_width());
    let ratio = if progress.total > 0 {
        (progress.done as f32 / progress.total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let percent = (ratio * 100.0).round() as u32;
    egui::Frame::default()
        .fill(crate::ui::theme::palette(ui).surface)
        .stroke(egui::Stroke::new(
            1.0,
            crate::ui::theme::palette(ui).surface_hover,
        ))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(width);
            if progress.waiting {
                // En attente de l'acceptation du destinataire (média au-delà du seuil d'accord).
                ui.label(
                    egui::RichText::new(format!(
                        "⏳ En attente d'envoi : {}",
                        elide(&media.filename, 32)
                    ))
                    .strong(),
                );
                ui.label(
                    egui::RichText::new(format_bytes(progress.total))
                        .small()
                        .color(crate::ui::theme::palette(ui).text_muted),
                );
                return;
            }
            ui.label(egui::RichText::new(elide(&media.filename, 38)).strong());
            ui.add_space(6.0);
            ui.add(
                egui::ProgressBar::new(ratio)
                    .desired_width(width)
                    .corner_radius(4.0),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "{percent} %  |  {} / {}",
                    format_bytes(progress.done),
                    format_bytes(progress.total)
                ))
                .small()
                .color(crate::ui::theme::palette(ui).text_muted),
            );
        });
}

/// Rend un média dans le fil et renvoie l'action utilisateur éventuelle.
pub(crate) fn render_media_block(
    ui: &mut egui::Ui,
    media: &MediaAttachment,
    texture: Option<&egui::TextureHandle>,
) -> Option<MediaAction> {
    // GIF : animé via les loaders egui_extras, chargé depuis l'URL Klipy.
    // Taille forcée d'après le ratio HD pour un affichage net et non riquiqui.
    // Gel hors écran : la place est réservée (pas de saut de layout), mais le
    // widget animé n'est émis que si le rectangle intersecte le viewport —
    // un GIF invisible ne décode rien, ne s'anime pas et ne déclenche aucun
    // repaint. Dès qu'un pixel entre à l'écran, il s'anime immédiatement.
    if media.kind == MediaKind::Gif {
        // Filtre appliqué ici, au point de chargement : il couvre aussi les
        // messages déjà en base, reçus avant l'ajout de ce contrôle.
        if let Some(url) = media
            .url
            .as_ref()
            .filter(|url| crate::message::media_url_is_loadable(url))
        {
            let max_w = GIF_MAX_WIDTH.min(ui.available_width());
            let size = gif_display_size(media.width, media.height, max_w, GIF_MAX_HEIGHT);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                ui.put(
                    rect,
                    egui::Image::from_uri(url.clone())
                        .fit_to_exact_size(size)
                        .corner_radius(8.0),
                );
            }
            return None;
        }
        return file_card(ui, media);
    }
    match (&media.kind, texture) {
        (MediaKind::Image, Some(texture)) => {
            let width = fitted_width(texture, THUMB_MAX_WIDTH, THUMB_MAX_HEIGHT);
            let image = egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                texture.size_vec2(),
            ))
            .max_width(width)
            .maintain_aspect_ratio(true)
            .corner_radius(8.0)
            .sense(egui::Sense::click());
            let response = ui
                .add(image)
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            response.clicked().then_some(MediaAction::View)
        }
        // Image dont le cache est manquant → repli en carte fichier.
        _ => file_card(ui, media),
    }
}

/// Largeur de la carte fichier dans le fil (façon Discord).
const FILE_CARD_WIDTH: f32 = 380.0;

/// Carte d'un fichier non-image (ou image indisponible), façon Discord : badge
/// d'extension coloré, nom mis en valeur, taille, et bouton de téléchargement.
fn file_card(ui: &mut egui::Ui, media: &MediaAttachment) -> Option<MediaAction> {
    let mut action = None;
    let width = FILE_CARD_WIDTH.min(ui.available_width());
    egui::Frame::default()
        .fill(crate::ui::theme::palette(ui).surface)
        .stroke(egui::Stroke::new(
            1.0,
            crate::ui::theme::palette(ui).surface_hover,
        ))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let extension = extension_lower(&media.filename).unwrap_or_default();
                file_badge(ui, &extension);
                ui.vertical(|ui| {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(elide(&media.filename, 38))
                            .color(crate::ui::theme::palette(ui).link)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format_bytes(media.size_bytes))
                            .small()
                            .color(crate::ui::theme::palette(ui).text_muted),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if download_button(ui) {
                        action = Some(MediaAction::Download);
                    }
                });
            });
        });
    action
}

/// Badge carré coloré portant l'extension du fichier (ou une icône générique).
fn file_badge(ui: &mut egui::Ui, extension: &str) {
    let size = 40.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(8),
        crate::ui::theme::palette(ui).accent,
    );
    let label: String = extension.chars().take(4).collect::<String>().to_uppercase();
    let text = if label.is_empty() {
        "FILE".to_string()
    } else {
        label
    };
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(11.0),
        egui::Color32::WHITE,
    );
}

/// Bouton de téléchargement peint (flèche vers un bac). Renvoie `true` au clic.
fn download_button(ui: &mut egui::Ui) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let painter = ui.painter();
        if hovered {
            painter.rect_filled(
                rect,
                egui::CornerRadius::same(6),
                crate::ui::theme::palette(ui).surface_hover,
            );
        }
        let color = if hovered {
            egui::Color32::WHITE
        } else {
            crate::ui::theme::palette(ui).text_muted
        };
        let stroke = egui::Stroke::new(1.7, color);
        let c = rect.center();
        // Flèche : tige verticale + pointe.
        painter.line_segment(
            [c + egui::vec2(0.0, -6.0), c + egui::vec2(0.0, 3.0)],
            stroke,
        );
        painter.line_segment(
            [c + egui::vec2(-4.0, -1.0), c + egui::vec2(0.0, 3.0)],
            stroke,
        );
        painter.line_segment(
            [c + egui::vec2(4.0, -1.0), c + egui::vec2(0.0, 3.0)],
            stroke,
        );
        // Bac de réception.
        painter.line_segment(
            [c + egui::vec2(-6.0, 7.0), c + egui::vec2(6.0, 7.0)],
            stroke,
        );
    }
    response.on_hover_text("Télécharger").clicked()
}

/// Raccourcit un texte trop long avec une ellipse finale.
pub(crate) fn elide(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let kept: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// UVs de recadrage centré pour remplir un carré `target_w`x`target_h` à
/// partir d'une texture `tex_size`, en préservant le centre de l'image source
/// (portrait : rogne haut/bas ; paysage : rogne gauche/droite).
fn center_crop_uv(tex_size: egui::Vec2, target_w: f32, target_h: f32) -> (egui::Pos2, egui::Pos2) {
    if tex_size.x <= 0.0 || tex_size.y <= 0.0 || target_w <= 0.0 || target_h <= 0.0 {
        return (egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    }
    let tex_ratio = tex_size.x / tex_size.y;
    let target_ratio = target_w / target_h;
    if tex_ratio > target_ratio {
        // Image plus large que la cible : rogner les côtés.
        let visible_frac = target_ratio / tex_ratio;
        let margin = (1.0 - visible_frac) / 2.0;
        (egui::pos2(margin, 0.0), egui::pos2(1.0 - margin, 1.0))
    } else {
        // Image plus haute (ou égale) que la cible : rogner haut/bas.
        let visible_frac = tex_ratio / target_ratio;
        let margin = (1.0 - visible_frac) / 2.0;
        (egui::pos2(0.0, margin), egui::pos2(1.0, 1.0 - margin))
    }
}

/// Vignette carrée compacte (façon Discord) pour un aperçu de réponse,
/// recadrée au centre — contrairement à `render_media_block`, qui préserve le
/// ratio complet de l'image.
pub(crate) fn render_reply_thumb(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    size: f32,
) {
    let Some(texture) = texture else {
        return;
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let (uv_min, uv_max) = center_crop_uv(texture.size_vec2(), size, size);
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(uv_min, uv_max),
        egui::Color32::WHITE,
    );
}

/// Côté maximal (px) d'une texture affichée dans le fil. Le rendu y est borné
/// à ~320 px : 1024 couvre largement les écrans retina sans stocker la pleine
/// résolution (une photo 12 Mpx ferait ~48 Mo de texture GPU). La visionneuse
/// charge sa propre texture pleine résolution, libérée à sa fermeture.
const FEED_TEXTURE_MAX_PX: u32 = 1024;
/// Nombre maximal de textures médias conservées en cache (éviction LRU).
const MEDIA_TEXTURE_CACHE_MAX: usize = 32;

impl AbcomApp {
    /// Texture d'un média image pour le fil : chargée paresseusement,
    /// réduite à [`FEED_TEXTURE_MAX_PX`], mise en cache avec éviction LRU.
    pub(crate) fn media_texture(
        &mut self,
        ctx: &egui::Context,
        id: &str,
    ) -> Option<egui::TextureHandle> {
        if let Some(cached) = self.media.textures.get(id) {
            let cached = cached.clone();
            self.touch_media_texture(id);
            return cached;
        }
        let texture = self.load_media_texture(ctx, id, Some(FEED_TEXTURE_MAX_PX));
        self.media.textures.insert(id.to_string(), texture.clone());
        self.touch_media_texture(id);
        self.evict_media_textures();
        texture
    }

    /// Place `id` en tête de l'ordre d'accès LRU.
    fn touch_media_texture(&mut self, id: &str) {
        if let Some(pos) = self.media.texture_lru.iter().position(|x| x == id) {
            self.media.texture_lru.remove(pos);
        }
        self.media.texture_lru.push(id.to_string());
    }

    /// Évince les textures les moins récemment affichées au-delà du plafond
    /// (les handles droppés libèrent la mémoire GPU côté egui).
    fn evict_media_textures(&mut self) {
        while self.media.texture_lru.len() > MEDIA_TEXTURE_CACHE_MAX {
            let oldest = self.media.texture_lru.remove(0);
            self.media.textures.remove(&oldest);
        }
    }

    /// Texture pleine résolution pour la visionneuse, conservée uniquement
    /// tant qu'elle est ouverte.
    fn viewer_texture_for(&mut self, ctx: &egui::Context, id: &str) -> Option<egui::TextureHandle> {
        if let Some((cached_id, texture)) = &self.media.viewer_texture {
            if cached_id == id {
                return Some(texture.clone());
            }
        }
        let texture = self.load_media_texture(ctx, id, None)?;
        self.media.viewer_texture = Some((id.to_string(), texture.clone()));
        Some(texture)
    }

    fn load_media_texture(
        &self,
        ctx: &egui::Context,
        id: &str,
        max_px: Option<u32>,
    ) -> Option<egui::TextureHandle> {
        // Le chemin se calcule sous verrou, la lecture se fait sans : le
        // fichier peut peser plusieurs centaines de Mio et `AppState` est le
        // verrou global — le tenir pendant l'E/S bloquerait tout le reste.
        let path = self.state.lock_safe().media_path(id);
        let bytes = std::fs::read(path).ok()?;
        let mut image = crate::util::decode_image_bounded(&bytes)?;
        let name = match max_px {
            Some(max) => {
                if image.width().max(image.height()) > max {
                    image = image.thumbnail(max, max);
                }
                format!("media_{id}")
            }
            None => format!("media_full_{id}"),
        };
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            rgba.as_raw(),
        );
        Some(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR))
    }

    /// Nom d'origine d'un média retrouvé dans l'historique (sinon son `id`).
    fn media_filename(&self, id: &str) -> String {
        let s = self.state.lock_safe();
        s.messages
            .iter()
            .filter_map(|m| m.media.as_ref())
            .find(|m| m.id == id)
            .map(|m| m.filename.clone())
            .unwrap_or_else(|| id.to_string())
    }

    /// Copie un média vers le dossier Téléchargements (sans écraser un fichier
    /// existant : un suffixe « (n) » est ajouté au besoin).
    pub(crate) fn download_media(&mut self, id: &str, filename: &str) {
        let Some(dir) = dirs::download_dir() else {
            self.notify(self.t(i18n::DOSSIER_TELECHARGEMENTS_INTROUVABLE));
            return;
        };
        let src = self.state.lock_safe().media_path(id);
        let dest = unique_destination(&dir, filename);
        let event_tx = self.net.event_tx.clone();
        let filename = filename.to_string();
        // Thread dédié : un média va jusqu'à 2 Gio et `fs::copy` est bloquant.
        // Sur le thread UI, la fenêtre gèlerait le temps de la copie.
        std::thread::spawn(move || {
            let result = std::fs::copy(&src, &dest);
            let filename = match result {
                Ok(_) => dest
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .or(Some(filename)),
                Err(e) => {
                    tracing::warn!("téléchargement échoué ({filename}): {e}");
                    None
                }
            };
            let _ = event_tx.blocking_send(AppEvent::MediaDownloaded { filename });
        });
    }

    /// Visionneuse plein écran : image agrandie + bouton de téléchargement.
    pub(crate) fn show_media_viewer(&mut self, ctx: &egui::Context) {
        let Some(id) = self.media.viewer.clone() else {
            // Visionneuse fermée : libère la texture pleine résolution.
            self.media.viewer_texture = None;
            return;
        };
        let texture = self.viewer_texture_for(ctx, &id);
        let filename = self.media_filename(&id);
        let title = self.t(i18n::APERCU);
        let download_label = self.t(i18n::TELECHARGER);
        let unavailable = self.t(i18n::APERCU_INDISPONIBLE);

        let mut open = true;
        let mut download = false;
        let screen = ctx.viewport_rect();
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                if let Some(texture) = &texture {
                    let width = fitted_width(texture, screen.width() * 0.85, screen.height() * 0.8);
                    ui.add(
                        egui::Image::new(egui::load::SizedTexture::new(
                            texture.id(),
                            texture.size_vec2(),
                        ))
                        .max_width(width)
                        .maintain_aspect_ratio(true)
                        .corner_radius(8.0),
                    );
                } else {
                    ui.label(unavailable);
                }
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if ui.button(download_label).clicked() {
                        download = true;
                    }
                });
            });

        if download {
            self.download_media(&id, &filename);
        }
        if !open {
            self.media.viewer = None;
            self.media.viewer_texture = None;
        }
    }

    fn notify(&mut self, text: &str) {
        self.notify_owned(text.to_string());
    }

    fn notify_owned(&mut self, text: String) {
        self.last_notification = Some(text);
        self.notification_time = std::time::Instant::now();
    }
}

/// Destination non conflictuelle dans `dir` : `nom.ext`, puis `nom (1).ext`, …
fn unique_destination(dir: &std::path::Path, filename: &str) -> std::path::PathBuf {
    // `filename` provient du réseau : on ne garde que le dernier composant pour
    // rester dans `dir` (défense contre le path traversal, p. ex. « ../../… »).
    let filename = std::path::Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("fichier");
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match filename.rsplit_once('.') {
        Some((stem, ext)) => (stem.to_string(), format!(".{ext}")),
        None => (filename.to_string(), String::new()),
    };
    for n in 1.. {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

#[cfg(test)]
#[path = "../tests/test_ui_media.rs"]
mod tests;
