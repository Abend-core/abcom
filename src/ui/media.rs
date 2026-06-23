//! Affichage des médias dans le fil : vignette d'image cliquable, carte fichier
//! téléchargeable, visionneuse plein écran et téléchargement vers le dossier
//! Téléchargements du système.

use eframe::egui;

use crate::message::{MediaAttachment, MediaKind};

use super::chat_panel::format_bytes;
use super::AbcomApp;

/// Largeur maximale d'une vignette d'image dans le fil.
const THUMB_MAX_WIDTH: f32 = 320.0;
/// Hauteur maximale d'une vignette d'image dans le fil.
const THUMB_MAX_HEIGHT: f32 = 260.0;

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

/// Rend un média dans le fil et renvoie l'action utilisateur éventuelle.
pub(crate) fn render_media_block(
    ui: &mut egui::Ui,
    media: &MediaAttachment,
    texture: Option<&egui::TextureHandle>,
) -> Option<MediaAction> {
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

/// Carte d'un fichier non-image (ou image indisponible) : nom, taille, bouton.
fn file_card(ui: &mut egui::Ui, media: &MediaAttachment) -> Option<MediaAction> {
    let mut action = None;
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(48, 52, 60))
        .corner_radius(egui::CornerRadius::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("📄").size(22.0));
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&media.filename).strong());
                    ui.label(
                        egui::RichText::new(format_bytes(media.size_bytes))
                            .small()
                            .weak(),
                    );
                });
                if ui.button("⬇").on_hover_text("Télécharger").clicked() {
                    action = Some(MediaAction::Download);
                }
            });
        });
    action
}

impl AbcomApp {
    /// Texture d'un média image, chargée paresseusement puis mise en cache.
    pub(crate) fn media_texture(
        &mut self,
        ctx: &egui::Context,
        id: &str,
    ) -> Option<egui::TextureHandle> {
        if let Some(cached) = self.media_textures.get(id) {
            return cached.clone();
        }
        let texture = self.load_media_texture(ctx, id);
        self.media_textures.insert(id.to_string(), texture.clone());
        texture
    }

    fn load_media_texture(&self, ctx: &egui::Context, id: &str) -> Option<egui::TextureHandle> {
        let bytes = self.state.lock().unwrap().media_bytes(id)?;
        let image = image::load_from_memory(&bytes).ok()?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            rgba.as_raw(),
        );
        Some(ctx.load_texture(format!("media_{id}"), color_image, egui::TextureOptions::LINEAR))
    }

    /// Nom d'origine d'un média retrouvé dans l'historique (sinon son `id`).
    fn media_filename(&self, id: &str) -> String {
        let s = self.state.lock().unwrap();
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
            self.notify(self.tr(
                "Dossier Téléchargements introuvable",
                "Downloads folder not found",
            ));
            return;
        };
        let src = self.state.lock().unwrap().media_path(id);
        let dest = unique_destination(&dir, filename);
        match std::fs::copy(&src, &dest) {
            Ok(_) => {
                let name = dest
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(filename)
                    .to_string();
                let msg = format!("{} {}", self.tr("Téléchargé :", "Downloaded:"), name);
                self.notify_owned(msg);
            }
            Err(e) => {
                eprintln!("[ui] Téléchargement échoué ({}): {}", filename, e);
                self.notify(self.tr("Téléchargement impossible", "Download failed"));
            }
        }
    }

    /// Visionneuse plein écran : image agrandie + bouton de téléchargement.
    pub(crate) fn show_media_viewer(&mut self, ctx: &egui::Context) {
        let Some(id) = self.media_viewer.clone() else {
            return;
        };
        let texture = self.media_texture(ctx, &id);
        let filename = self.media_filename(&id);
        let title = self.tr("Aperçu", "Preview");
        let download_label = self.tr("⬇ Télécharger", "⬇ Download");
        let unavailable = self.tr("Aperçu indisponible", "Preview unavailable");

        let mut open = true;
        let mut download = false;
        let screen = ctx.screen_rect();
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
            self.media_viewer = None;
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
mod tests {
    use super::unique_destination;

    #[test]
    fn unique_destination_keeps_free_name() {
        let dir = std::env::temp_dir().join(format!("abcom_dl_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = unique_destination(&dir, "libre.txt");
        assert_eq!(dest, dir.join("libre.txt"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unique_destination_avoids_collision() {
        let dir = std::env::temp_dir().join(format!("abcom_dl2_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("photo.png"), b"x").unwrap();
        let dest = unique_destination(&dir, "photo.png");
        assert_eq!(dest, dir.join("photo (1).png"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
