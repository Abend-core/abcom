//! Client de l'API Klipy (GIF + Mèmes) — recherche, tendances et pagination.
//!
//! Le transport choisi est « URL uniquement » : on ne télécharge jamais les
//! octets. Le sélecteur affiche les variantes **WebP sm/xs** (vignettes) et le
//! message transporte l'URL de la variante **WebP hd** (affichage dans le fil).
//! Les requêtes JSON partent via [`ehttp::fetch`] (non bloquant, callback sur un
//! thread dédié) ; le rendu animé est assuré par les loaders `egui_extras`.
//!
//! GIF et Mèmes sont chargés en parallèle et **entrelacés** dans l'affichage.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Deserialize;

const BASE: &str = "https://api.klipy.com/api/v1";
/// Nombre d'items demandés par page et par type (min 8, max 50 côté Klipy).
const PER_PAGE: u32 = 16;
/// Filtre de contenu par défaut (g / pg / pg-13 / r).
const RATING: &str = "pg-13";

/// Un GIF ou mème prêt à afficher : vignette (sm/xs webp) et version pleine (hd webp).
#[derive(Clone, Debug)]
pub struct GifItem {
    pub id: String,
    /// URL de la vignette animée affichée dans le sélecteur.
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
        let preview = variant(&self.file, &["sm", "xs", "md", "hd"])?;
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

/// Première variante WebP disponible parmi `sizes` (repli sur GIF si aucune WebP).
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

/// Parse une réponse Klipy en liste de [`GifItem`] et drapeau « page suivante ».
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

fn trending_url(key: &str, locale: &str, page: u32) -> String {
    format!("{BASE}/{key}/gifs/trending?page={page}&per_page={PER_PAGE}&rating={RATING}&locale={locale}")
}

fn search_url(key: &str, locale: &str, query: &str, page: u32) -> String {
    format!(
        "{BASE}/{key}/gifs/search?q={}&page={page}&per_page={PER_PAGE}&rating={RATING}&locale={locale}",
        percent_encode(query)
    )
}

fn meme_trending_url(key: &str, locale: &str, page: u32) -> String {
    format!("{BASE}/{key}/memes/trending?page={page}&per_page={PER_PAGE}&rating={RATING}&locale={locale}")
}

fn meme_search_url(key: &str, locale: &str, query: &str, page: u32) -> String {
    format!(
        "{BASE}/{key}/memes/search?q={}&page={page}&per_page={PER_PAGE}&rating={RATING}&locale={locale}",
        percent_encode(query)
    )
}

/// Encodage minimal pour le paramètre `q` (espaces et caractères non sûrs).
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

/// Entrelace deux slices en alternance (a[0], b[0], a[1], b[1], …),
/// puis ajoute le reste de la plus longue.
fn interleave(a: &[GifItem], b: &[GifItem]) -> Vec<GifItem> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let mut ai = a.iter();
    let mut bi = b.iter();
    loop {
        match (ai.next(), bi.next()) {
            (Some(x), Some(y)) => {
                out.push(x.clone());
                out.push(y.clone());
            }
            (Some(x), None) => {
                out.push(x.clone());
                out.extend(ai.cloned());
                break;
            }
            (None, Some(y)) => {
                out.push(y.clone());
                out.extend(bi.cloned());
                break;
            }
            (None, None) => break,
        }
    }
    out
}

// ─── Discriminant interne GIF vs Mème ───────────────────────────────────────

#[derive(Clone, Copy)]
enum Kind {
    Gif,
    Meme,
}

// ─── État partagé du flux de GIF + Mèmes ────────────────────────────────────

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
    /// Items bruts du endpoint GIF.
    pub gif_raw: Vec<GifItem>,
    /// Items bruts du endpoint Mème.
    pub meme_raw: Vec<GifItem>,
    pub status: GifStatus,
    /// Requête courante ; vide = tendances.
    pub query: String,
    pub gif_page: u32,
    pub meme_page: u32,
    pub gif_has_next: bool,
    pub meme_has_next: bool,
    /// Session courante : incrémentée à chaque nouvelle recherche/tendance.
    /// Les callbacks d'une session précédente sont ignorés.
    generation: u64,
    /// Nombre de requêtes encore en vol pour la session courante.
    pending: u8,
}

impl GifFeedState {
    /// Liste entrelacée GIF + Mèmes, prête pour l'affichage.
    pub fn items(&self) -> Vec<GifItem> {
        interleave(&self.gif_raw, &self.meme_raw)
    }

    /// Vrai si au moins un des deux feeds a une page suivante.
    pub fn has_next(&self) -> bool {
        self.gif_has_next || self.meme_has_next
    }
}

/// Flux de GIF+Mèmes partagé entre l'UI et les callbacks réseau (`ehttp`).
#[derive(Clone, Default)]
pub struct GifFeed {
    inner: Arc<Mutex<GifFeedState>>,
}

impl GifFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, GifFeedState> {
        self.inner.lock().unwrap()
    }

    /// Charge la première page des tendances GIF + Mèmes en parallèle.
    pub fn load_trending(&self, ctx: &egui::Context, key: &str, locale: &str) {
        let gen = {
            let mut s = self.inner.lock().unwrap();
            s.query.clear();
            s.gif_page = 1;
            s.meme_page = 1;
            s.generation += 1;
            s.gif_raw.clear();
            s.meme_raw.clear();
            s.gif_has_next = false;
            s.meme_has_next = false;
            s.pending = 2;
            s.status = GifStatus::Loading;
            s.generation
        };
        self.fire_one(ctx, trending_url(key, locale, 1), gen, Kind::Gif);
        self.fire_one(ctx, meme_trending_url(key, locale, 1), gen, Kind::Meme);
    }

    /// Lance une recherche GIF + Mèmes. Query vide → tendances.
    pub fn search(&self, ctx: &egui::Context, key: &str, locale: &str, query: &str) {
        if query.trim().is_empty() {
            self.load_trending(ctx, key, locale);
            return;
        }
        let gen = {
            let mut s = self.inner.lock().unwrap();
            s.query = query.to_string();
            s.gif_page = 1;
            s.meme_page = 1;
            s.generation += 1;
            s.gif_raw.clear();
            s.meme_raw.clear();
            s.gif_has_next = false;
            s.meme_has_next = false;
            s.pending = 2;
            s.status = GifStatus::Loading;
            s.generation
        };
        self.fire_one(ctx, search_url(key, locale, query, 1), gen, Kind::Gif);
        self.fire_one(ctx, meme_search_url(key, locale, query, 1), gen, Kind::Meme);
    }

    /// Charge la page suivante pour chaque feed qui a encore des résultats.
    pub fn load_more(&self, ctx: &egui::Context, key: &str, locale: &str) {
        let (gen, gif_url, meme_url) = {
            let mut s = self.inner.lock().unwrap();
            if s.status == GifStatus::Loading {
                return;
            }
            let need_gif = s.gif_has_next;
            let need_meme = s.meme_has_next;
            if !need_gif && !need_meme {
                return;
            }
            if need_gif {
                s.gif_page += 1;
            }
            if need_meme {
                s.meme_page += 1;
            }
            s.pending = (need_gif as u8) + (need_meme as u8);
            s.status = GifStatus::Loading;
            let gen = s.generation;
            let gif_url = need_gif.then(|| {
                if s.query.is_empty() {
                    trending_url(key, locale, s.gif_page)
                } else {
                    search_url(key, locale, &s.query, s.gif_page)
                }
            });
            let meme_url = need_meme.then(|| {
                if s.query.is_empty() {
                    meme_trending_url(key, locale, s.meme_page)
                } else {
                    meme_search_url(key, locale, &s.query, s.meme_page)
                }
            });
            (gen, gif_url, meme_url)
        };
        if let Some(url) = gif_url {
            self.fire_one(ctx, url, gen, Kind::Gif);
        }
        if let Some(url) = meme_url {
            self.fire_one(ctx, url, gen, Kind::Meme);
        }
    }

    /// Émet une requête HTTP et applique le résultat au sous-feed correspondant.
    /// `expected_gen` : si la session a changé entre temps, la réponse est ignorée.
    fn fire_one(&self, ctx: &egui::Context, url: String, expected_gen: u64, kind: Kind) {
        let inner = self.inner.clone();
        let ctx = ctx.clone();
        ehttp::fetch(ehttp::Request::get(url), move |result| {
            let mut s = inner.lock().unwrap();
            if s.generation != expected_gen {
                return;
            }
            s.pending = s.pending.saturating_sub(1);
            match result {
                Ok(resp) if resp.ok => match parse(&resp.bytes) {
                    Ok((items, has_next)) => {
                        match kind {
                            Kind::Gif => {
                                s.gif_raw.extend(items);
                                s.gif_has_next = has_next;
                            }
                            Kind::Meme => {
                                s.meme_raw.extend(items);
                                s.meme_has_next = has_next;
                            }
                        }
                        if s.pending == 0 {
                            s.status = GifStatus::Loaded;
                        }
                    }
                    Err(e) => {
                        eprintln!("[klipy] parsing réponse échoué : {e}");
                        if s.pending == 0 {
                            s.status = GifStatus::Error(e);
                        }
                    }
                },
                Ok(resp) => {
                    eprintln!("[klipy] HTTP {} : {}", resp.status, resp.status_text);
                    if s.pending == 0 {
                        s.status = GifStatus::Error(format!("HTTP {}", resp.status));
                    }
                }
                Err(e) => {
                    eprintln!("[klipy] requête échouée : {e}");
                    if s.pending == 0 {
                        s.status = GifStatus::Error(e);
                    }
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
        let url = trending_url("MYKEY", "fr", 2);
        assert!(url.contains("/MYKEY/gifs/trending"));
        assert!(url.contains("page=2"));
        assert!(url.contains(&format!("per_page={PER_PAGE}")));
        assert!(url.contains("locale=fr"));
    }

    #[test]
    fn meme_trending_url_uses_memes_endpoint() {
        let url = meme_trending_url("MYKEY", "fr", 1);
        assert!(url.contains("/MYKEY/memes/trending"));
        assert!(url.contains("page=1"));
    }

    #[test]
    fn search_url_encodes_query() {
        let url = search_url("K", "en", "happy cat", 1);
        assert!(url.contains("/K/gifs/search"));
        assert!(url.contains("q=happy%20cat"));
        assert!(url.contains("page=1"));
    }

    #[test]
    fn meme_search_url_encodes_query() {
        let url = meme_search_url("K", "en", "funny dog", 1);
        assert!(url.contains("/K/memes/search"));
        assert!(url.contains("q=funny%20dog"));
    }

    #[test]
    fn interleave_alternates_items() {
        let mk = |id: &str| GifItem {
            id: id.to_string(),
            preview_url: String::new(),
            full_url: String::new(),
            width: None,
            height: None,
        };
        let gifs = vec![mk("g1"), mk("g2"), mk("g3")];
        let memes = vec![mk("m1"), mk("m2")];
        let merged = interleave(&gifs, &memes);
        assert_eq!(
            merged.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["g1", "m1", "g2", "m2", "g3"]
        );
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
        let (items, has_next) = parse(body).unwrap();
        assert!(!has_next);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].preview_url, "https://x/sm.gif");
        assert_eq!(items[0].full_url, "https://x/sm.gif");
    }

    #[test]
    fn parse_skips_items_without_files() {
        let body = br#"{"data":{"data":[{"id":"empty","file":{}}],"has_next":false}}"#;
        let (items, _) = parse(body).unwrap();
        assert!(items.is_empty());
    }
}
