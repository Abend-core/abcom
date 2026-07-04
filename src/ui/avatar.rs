//! Avatars côté interface : normalisation des images choisies et rendu.
//!
//! - Les images sélectionnées par l'utilisateur sont recadrées en carré et
//!   réduites à une taille fixe avant d'être encodées en PNG, ce qui borne la
//!   taille transmise sur le réseau et garde un rendu net.
//! - Les textures egui sont mises en cache par nom d'utilisateur et rechargées
//!   à la volée quand les octets changent.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use eframe::egui;

use crate::message::{AvatarAnnounce, AvatarRequest};

use super::AbcomApp;

/// Côté du carré final de l'avatar, en pixels.
const AVATAR_PX: u32 = 256;
/// Proportion du côté utilisée comme rayon de coin (carré légèrement arrondi).
const AVATAR_CORNER_FACTOR: f32 = 0.22;

/// Rayon de coin d'un avatar pour une taille donnée (carré à coins arrondis).
fn avatar_corner(size: f32) -> egui::CornerRadius {
    egui::CornerRadius::same((size * AVATAR_CORNER_FACTOR).round() as u8)
}

/// Charge une image (raster ou SVG) depuis le disque et renvoie un PNG carré
/// normalisé, prêt à être affiché et partagé sur le réseau.
pub(crate) fn load_normalized_avatar(path: &Path) -> anyhow::Result<Vec<u8>> {
    let is_svg = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    let image = if is_svg {
        #[cfg(feature = "avatar-svg")]
        {
            rasterize_svg(&std::fs::read(path)?)?
        }
        #[cfg(not(feature = "avatar-svg"))]
        {
            anyhow::bail!("support SVG non compilé (feature `avatar-svg`)")
        }
    } else {
        image::open(path)?
    };

    // `resize_to_fill` couvre puis recadre au centre : pas de déformation.
    let square = image.resize_to_fill(AVATAR_PX, AVATAR_PX, image::imageops::FilterType::Lanczos3);

    let mut png = std::io::Cursor::new(Vec::new());
    square.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(png.into_inner())
}

/// Rasterise un SVG en image RGBA via resvg/usvg, à sa taille intrinsèque.
#[cfg(feature = "avatar-svg")]
fn rasterize_svg(data: &[u8]) -> anyhow::Result<image::DynamicImage> {
    let tree = resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default())?;
    let size = tree.size().to_int_size();
    let (width, height) = (size.width().max(1), size.height().max(1));
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("dimensions SVG invalides"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let buffer = image::RgbaImage::from_raw(width, height, pixmap.take())
        .ok_or_else(|| anyhow::anyhow!("conversion du SVG impossible"))?;
    Ok(image::DynamicImage::ImageRgba8(buffer))
}

/// Couleur de pastille déterministe pour les avatars sans image (initiale).
pub(crate) fn placeholder_color(name: &str) -> egui::Color32 {
    const PALETTE: [egui::Color32; 6] = [
        egui::Color32::from_rgb(88, 101, 242),  // bleu Discord
        egui::Color32::from_rgb(87, 242, 135),  // vert
        egui::Color32::from_rgb(254, 231, 92),  // jaune
        egui::Color32::from_rgb(235, 69, 158),  // rose
        egui::Color32::from_rgb(255, 138, 76),  // orange
        egui::Color32::from_rgb(116, 200, 255), // cyan
    ];
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    PALETTE[(hasher.finish() as usize) % PALETTE.len()]
}

/// Dessine un avatar carré à coins légèrement arrondis : image si disponible,
/// sinon une pastille colorée portant l'initiale du nom. Occupe un carré de
/// `size` × `size`.
pub(crate) fn show_avatar(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    name: &str,
    size: f32,
) {
    let corner = avatar_corner(size);
    match texture {
        Some(texture) => {
            ui.add_sized(
                [size, size],
                egui::Image::new(egui::load::SizedTexture::new(
                    texture.id(),
                    egui::vec2(size, size),
                ))
                .corner_radius(corner),
            );
        }
        None => {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
            if !ui.is_rect_visible(rect) {
                return;
            }
            let painter = ui.painter();
            painter.rect_filled(rect, corner, placeholder_color(name));
            let initial = name
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_else(|| "?".to_string());
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                initial,
                egui::FontId::proportional(size * 0.42),
                egui::Color32::WHITE,
            );
        }
    }
}

impl AbcomApp {
    /// Texture de l'avatar d'un utilisateur, chargée paresseusement puis mise
    /// en cache. Renvoie `None` si l'utilisateur n'a pas d'avatar.
    pub(crate) fn avatar_texture(
        &mut self,
        ctx: &egui::Context,
        username: &str,
    ) -> Option<egui::TextureHandle> {
        if let Some(texture) = self.avatar_textures.get(username) {
            return Some(texture.clone());
        }

        let bytes = self.state.lock().unwrap().avatar_bytes(username)?;
        let image = image::load_from_memory(&bytes).ok()?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            rgba.as_raw(),
        );
        let texture = ctx.load_texture(
            format!("avatar_{username}"),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.avatar_textures
            .insert(username.to_string(), texture.clone());
        Some(texture)
    }

    /// Diffuse notre avatar courant à tous les pairs en ligne. Utilisé après un
    /// changement d'avatar ; un PNG vide signale le retrait aux destinataires.
    pub(crate) fn broadcast_my_avatar(&mut self) {
        let (my_name, png) = {
            let s = self.state.lock().unwrap();
            (
                s.my_username.clone(),
                s.my_avatar.clone().unwrap_or_default(),
            )
        };
        let announce = AvatarAnnounce { from: my_name, png };
        self.send_avatar_announce(announce);
    }

    /// Envoie une annonce d'avatar à chaque pair en ligne et mémorise l'envoi
    /// pour éviter les répétitions lors des prochaines découvertes.
    fn send_avatar_announce(&mut self, announce: AvatarAnnounce) {
        let online: Vec<(String, std::net::SocketAddr)> = {
            let s = self.state.lock().unwrap();
            s.peers
                .iter()
                .filter(|p| p.online)
                .map(|p| (p.username.clone(), p.addr))
                .collect()
        };
        self.avatar_sent_to.clear();
        for (username, addr) in online {
            let request = AvatarRequest {
                to_addr: addr,
                announce: announce.clone(),
            };
            if self.send_avatar_tx.try_send(request).is_ok() {
                self.avatar_sent_to.insert(username);
            }
        }
    }
}
