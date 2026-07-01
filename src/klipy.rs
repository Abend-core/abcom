//! Client de l'API Klipy — GIF, Mèmes statiques et Stickers.
//!
//! Le transport choisi est « URL uniquement » : on ne télécharge jamais les
//! octets. Le sélecteur affiche les variantes **WebP sm/xs** (vignettes) et le
//! message transporte l'URL de la variante **WebP hd** (affichage dans le fil).
//! Les requêtes JSON partent via [`ehttp::fetch`] (non bloquant, callback sur un
//! thread dédié) ; le rendu animé est assuré par les loaders `egui_extras`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Deserialize;

const BASE: &str = "https://api.klipy.com/api/v1";
/// Nombre d'items demandés par page (min 8, max 50 côté Klipy).
const PER_PAGE: u32 = 24;
/// Filtre de contenu par défaut (g / pg / pg-13 / r).
const RATING: &str = "pg-13";

/// Type de contenu géré par un [`GifFeed`].
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentKind {
    #[default]
    Gif,
    Meme,
    Sticker,
}

/// Un GIF, mème ou sticker prêt à afficher.
#[derive(Clone, Debug)]
pub struct GifItem {
    pub id: String,
    /// URL de la vignette affichée dans le sélecteur.
    pub preview_url: String,
    /// URL de la version pleine envoyée dans le message.
    pub full_url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

// ─── Désérialisation de la réponse Klipy ────────────────────────────────────

#[derive(Deserialize)]
struct KlipyResponse {
    #[serde(default)]
    data: KlipyData,
}

#[derive(Deserialize, Default)]
struct KlipyData {
    #[serde(default)]
    data: Vec<KlipyGif>,
    #[serde(default)]
    has_next: bool,
}

#[derive(Deserialize, Default)]
struct KlipyGif {
    #[serde(default)]
    id: serde_json::Value,
    #[serde(default, alias = "files")]
    file: HashMap<String, SizeVariant>,
}

#[derive(Deserialize, Default)]
struct SizeVariant {
    #[serde(default)]
    webp: Option<FileMeta>,
    #[serde(default)]
    gif: Option<FileMeta>,
}

#[derive(Deserialize, Default)]
struct FileMeta {
    url: String,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

impl KlipyGif {
    fn to_item(&self) -> Option<GifItem> {
        let preview = variant(&self.file, &["xs", "sm", "md", "hd"])?;
        let full = variant(&self.file, &["hd", "md", "sm", "xs"])?;
        let id = value_to_string(&self.id).unwrap_or_else(|| full.url.clone());
        Some(GifItem {
            id,
            preview_url: preview.url.clone(),
            full_url: full.url.clone(),
            width: full.width,
            height: full.height,
        })
    }
}

fn variant<'a>(file: &'a HashMap<String, SizeVariant>, sizes: &[&str]) -> Option<&'a FileMeta> {
    for s in sizes {
        if let Some(meta) = file.get(*s).and_then(|v| v.webp.as_ref()) {
            if !meta.url.is_empty() {
                return Some(meta);
            }
        }
    }
    for s in sizes {
        if let Some(meta) = file.get(*s).and_then(|v| v.gif.as_ref()) {
            if !meta.url.is_empty() {
                return Some(meta);
            }
        }
    }
    None
}

fn value_to_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse(body: &[u8]) -> Result<(Vec<GifItem>, bool), String> {
    let resp: KlipyResponse = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let items = resp
        .data
        .data
        .iter()
        .filter_map(KlipyGif::to_item)
        .collect();
    Ok((items, resp.data.has_next))
}

// ─── Construction des URL ───────────────────────────────────────────────────

fn trending_url(kind: ContentKind, key: &str, locale: &str, page: u32) -> String {
    let segment = kind_segment(kind);
    format!("{BASE}/{key}/{segment}/trending?page={page}&per_page={PER_PAGE}&rating={RATING}&locale={locale}")
}

fn search_url(kind: ContentKind, key: &str, locale: &str, query: &str, page: u32) -> String {
    let segment = kind_segment(kind);
    format!(
        "{BASE}/{key}/{segment}/search?q={}&page={page}&per_page={PER_PAGE}&rating={RATING}&locale={locale}",
        percent_encode(query)
    )
}

fn kind_segment(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Gif => "gifs",
        ContentKind::Meme => "static-memes",
        ContentKind::Sticker => "stickers",
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ─── État partagé ───────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Default)]
pub enum GifStatus {
    #[default]
    Idle,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Default)]
pub struct GifFeedState {
    pub items: Vec<GifItem>,
    pub status: GifStatus,
    pub query: String,
    pub page: u32,
    pub has_next: bool,
    generation: u64,
}

/// Flux de contenu Klipy (GIF, mème ou sticker) partagé entre l'UI et les
/// callbacks réseau. Le type de contenu est fixé à la création.
#[derive(Clone)]
pub struct GifFeed {
    inner: Arc<Mutex<GifFeedState>>,
    kind: ContentKind,
}

impl Default for GifFeed {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(GifFeedState::default())),
            kind: ContentKind::Gif,
        }
    }
}

impl GifFeed {
    pub fn new(kind: ContentKind) -> Self {
        Self {
            inner: Arc::new(Mutex::new(GifFeedState::default())),
            kind,
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, GifFeedState> {
        self.inner.lock().unwrap()
    }

    pub fn load_trending(&self, ctx: &egui::Context, key: &str, locale: &str) {
        {
            let mut s = self.inner.lock().unwrap();
            s.query.clear();
            s.page = 1;
        }
        self.fire(ctx, trending_url(self.kind, key, locale, 1), true);
    }

    pub fn search(&self, ctx: &egui::Context, key: &str, locale: &str, query: &str) {
        if query.trim().is_empty() {
            self.load_trending(ctx, key, locale);
            return;
        }
        {
            let mut s = self.inner.lock().unwrap();
            s.query = query.to_string();
            s.page = 1;
        }
        self.fire(ctx, search_url(self.kind, key, locale, query, 1), true);
    }

    pub fn load_more(&self, ctx: &egui::Context, key: &str, locale: &str) {
        let (query, next_page) = {
            let mut s = self.inner.lock().unwrap();
            if s.status == GifStatus::Loading || !s.has_next {
                return;
            }
            s.page += 1;
            (s.query.clone(), s.page)
        };
        let url = if query.is_empty() {
            trending_url(self.kind, key, locale, next_page)
        } else {
            search_url(self.kind, key, locale, &query, next_page)
        };
        self.fire(ctx, url, false);
    }

    fn fire(&self, ctx: &egui::Context, url: String, replace: bool) {
        let generation = {
            let mut s = self.inner.lock().unwrap();
            s.generation += 1;
            s.status = GifStatus::Loading;
            if replace {
                s.items.clear();
            }
            s.generation
        };
        let inner = self.inner.clone();
        let ctx = ctx.clone();
        ehttp::fetch(ehttp::Request::get(url), move |result| {
            let mut s = inner.lock().unwrap();
            if generation != s.generation {
                return;
            }
            match result {
                Ok(resp) if resp.ok => match parse(&resp.bytes) {
                    Ok((items, has_next)) => {
                        s.items.extend(items);
                        s.has_next = has_next;
                        s.status = GifStatus::Loaded;
                    }
                    Err(e) => {
                        eprintln!("[klipy] parsing réponse échoué : {e}");
                        s.status = GifStatus::Error(e);
                    }
                },
                Ok(resp) => {
                    eprintln!("[klipy] HTTP {} : {}", resp.status, resp.status_text);
                    s.status = GifStatus::Error(format!("HTTP {}", resp.status));
                }
                Err(e) => {
                    eprintln!("[klipy] requête échouée : {e}");
                    s.status = GifStatus::Error(e);
                }
            }
            ctx.request_repaint();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trending_url_contains_key_and_pagination() {
        let url = trending_url(ContentKind::Gif, "MYKEY", "fr", 2);
        assert!(url.contains("/MYKEY/gifs/trending"));
        assert!(url.contains("page=2"));
        assert!(url.contains(&format!("per_page={PER_PAGE}")));
        assert!(url.contains("locale=fr"));
    }

    #[test]
    fn meme_trending_url_uses_static_memes_segment() {
        let url = trending_url(ContentKind::Meme, "K", "fr", 1);
        assert!(url.contains("/K/static-memes/trending"));
    }

    #[test]
    fn sticker_search_url_uses_stickers_segment() {
        let url = search_url(ContentKind::Sticker, "K", "en", "cute", 1);
        assert!(url.contains("/K/stickers/search"));
        assert!(url.contains("q=cute"));
    }

    #[test]
    fn search_url_encodes_query() {
        let url = search_url(ContentKind::Gif, "K", "en", "happy cat", 1);
        assert!(url.contains("/K/gifs/search"));
        assert!(url.contains("q=happy%20cat"));
        assert!(url.contains("page=1"));
    }

    #[test]
    fn parse_extracts_xs_and_hd_webp() {
        let body = br#"{
            "result": true,
            "data": {
                "data": [
                    {
                        "id": 123,
                        "file": {
                            "xs": {"webp": {"url": "https://x/xs.webp", "width": 90, "height": 60}},
                            "hd": {"webp": {"url": "https://x/hd.webp", "width": 480, "height": 320}}
                        }
                    }
                ],
                "current_page": 1,
                "per_page": 24,
                "has_next": true
            }
        }"#;
        let (items, has_next) = parse(body).unwrap();
        assert!(has_next);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "123");
        assert_eq!(items[0].preview_url, "https://x/xs.webp");
        assert_eq!(items[0].full_url, "https://x/hd.webp");
        assert_eq!(items[0].width, Some(480));
        assert_eq!(items[0].height, Some(320));
    }

    #[test]
    fn parse_falls_back_to_gif_when_no_webp() {
        let body = br#"{"data":{"data":[{"id":"abc","file":{"sm":{"gif":{"url":"https://x/sm.gif"}}}}],"has_next":false}}"#;
        let (items, _) = parse(body).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].preview_url, "https://x/sm.gif");
    }

    #[test]
    fn parse_skips_items_without_files() {
        let body = br#"{"data":{"data":[{"id":"empty","file":{}}],"has_next":false}}"#;
        let (items, _) = parse(body).unwrap();
        assert!(items.is_empty());
    }
}
