//! Caches de données dérivées pour le rendu, invalidés par le compteur de
//! génération de [`AppState`] : une frame ne re-dérive rien tant que l'état
//! n'a pas changé (pas de clone de conversation, pas de re-parse markdown,
//! pas de re-hash, pas de verrou long).

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use chrono::{Local, NaiveDate};
use eframe::egui;

use crate::app::{AppState, Peer, ReceiptDetail};
use crate::message::{ChatMessage, Group, ReactionEntry};

use super::chat_panel::{
    day_divider_label, header_time, message_day, peer_color_for, starts_new_group,
};
use super::markdown::{parse_message, ParsedMarkdown};
use super::theme;
use super::UiLanguage;

/// Seuils de repli des messages très longs : protège le coût de layout du
/// fil, qui croît linéairement (mesuré : ~14 ms pour 100 k caractères,
/// ~770 ms pour 8 Mo — un gel d'interface). Comptés en caractères Unicode.
const COLLAPSE_CHARS: usize = 4_000;
const COLLAPSE_LINES: usize = 60;
/// Aperçu affiché quand le message est replié.
const PREVIEW_CHARS: usize = 2_000;
const PREVIEW_LINES: usize = 30;

/// Message trop long pour être affiché entier d'emblée : aperçu pré-parsé et
/// dimensions totales pour le bouton « Afficher la suite ».
pub(crate) struct CollapseInfo {
    pub(crate) preview: Arc<ParsedMarkdown>,
    pub(crate) total_lines: usize,
    pub(crate) total_chars: usize,
}

fn collapse_info(content: &str, emoji_map: &HashMap<String, usize>) -> Option<CollapseInfo> {
    let total_chars = content.chars().count();
    let total_lines = content.lines().count();
    if total_chars <= COLLAPSE_CHARS && total_lines <= COLLAPSE_LINES {
        return None;
    }
    let mut preview: String = content
        .lines()
        .take(PREVIEW_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    if preview.chars().count() > PREVIEW_CHARS {
        preview = preview.chars().take(PREVIEW_CHARS).collect();
    }
    preview.push_str(" ...");
    Some(CollapseInfo {
        preview: Arc::new(parse_message(&preview, emoji_map)),
        total_lines,
        total_chars,
    })
}

/// Citation de réponse pré-résolue (le message d'origine n'est recherché
/// qu'à la reconstruction du cache, pas à chaque frame).
pub(crate) struct ReplyInfo {
    pub(crate) resolved: Option<ChatMessage>,
    pub(crate) author: String,
    pub(crate) author_color: egui::Color32,
}

/// Une ligne du fil, entièrement pré-dérivée.
pub(crate) struct ChatRow {
    pub(crate) msg: ChatMessage,
    pub(crate) hash: u64,
    /// Libellé du séparateur de date à afficher au-dessus de la ligne.
    pub(crate) day_divider: Option<String>,
    /// Libellé du jour de la ligne (même sans changement de jour) : affiché
    /// en tête de fenêtre tronquée pour situer la coupure dans le temps.
    pub(crate) day_label: Option<String>,
    /// Ouvre un groupe façon Discord (avatar + nom + heure).
    pub(crate) starts_group: bool,
    pub(crate) header_time: String,
    pub(crate) name_color: egui::Color32,
    /// Pour nos messages en 1-à-1 : (livré, lu). Toujours `None` en salon.
    pub(crate) receipt: Option<(bool, bool, bool)>,
    /// Salons et « Tous » : liste nominative reçu/lu (popup « … »), portée
    /// par l'en-tête de chaque groupe de messages, quel qu'en soit l'auteur.
    pub(crate) receipt_detail: Option<ReceiptDetail>,
    pub(crate) reply: Option<ReplyInfo>,
    pub(crate) reactions: Vec<ReactionEntry>,
    pub(crate) display_name: String,
    pub(crate) markdown: Arc<ParsedMarkdown>,
    /// `Some` pour les messages très longs : affichés repliés (aperçu +
    /// « Afficher la suite ») pour ne pas geler le layout du fil.
    pub(crate) collapse: Option<CollapseInfo>,
}

/// Cache du fil de la conversation sélectionnée.
#[derive(Default)]
pub(crate) struct ChatCache {
    generation: Option<u64>,
    conversation_key: Option<Option<String>>,
    language: Option<UiLanguage>,
    today: Option<NaiveDate>,
    /// Thème du cache : les couleurs d'auteur en dépendent, il faut donc le
    /// reconstruire quand l'utilisateur bascule clair/sombre.
    dark_mode: Option<bool>,
    pub(crate) rows: Arc<Vec<ChatRow>>,
    /// Auteurs uniques du fil (préchargement des avatars).
    pub(crate) authors: Vec<String>,
    /// Identifiants des médias image du fil (préchargement des textures).
    pub(crate) image_media_ids: Vec<String>,
    pub(crate) my_name: String,
    pub(crate) multi_person: bool,
    /// Nom d'affichage du pair en 1-à-1 (titre de la conversation).
    pub(crate) private_peer_display: Option<String>,
    /// URLs des GIFs présents dans le fil (éviction du cache d'images egui
    /// quand ils en sortent).
    pub(crate) gif_urls: HashSet<String>,
    /// Markdown memoïsé par hash de message : survit aux reconstructions,
    /// un message donné n'est parsé qu'une seule fois.
    markdown: HashMap<u64, Arc<ParsedMarkdown>>,
}

impl ChatCache {
    /// Invalide entièrement le cache (y compris le markdown memoïsé) : la
    /// détection « message 100 % emoji » dépend du registre d'emojis, chargé
    /// en arrière-plan après les premières frames.
    pub(crate) fn invalidate(&mut self) {
        self.generation = None;
        self.markdown.clear();
    }

    pub(crate) fn conversation(&self) -> Option<&str> {
        self.conversation_key.as_ref().and_then(|c| c.as_deref())
    }

    /// Reconstruit le cache si (génération | conversation | langue | jour) a
    /// changé. Renvoie `None` si rien n'a changé, `Some(conversation_changée)`
    /// après reconstruction (remise à zéro du fenêtrage par l'appelant).
    pub(crate) fn refresh(
        &mut self,
        s: &AppState,
        language: UiLanguage,
        emoji_map: &HashMap<String, usize>,
        dark_mode: bool,
    ) -> Option<bool> {
        let today = Local::now().date_naive();
        let conv_changed = self.conversation_key.as_ref() != Some(&s.selected_conversation);
        if !conv_changed
            && self.generation == Some(s.content_generation)
            && self.language == Some(language)
            && self.today == Some(today)
            && self.dark_mode == Some(dark_mode)
        {
            return None;
        }
        let palette = theme::for_dark_mode(dark_mode);
        self.dark_mode = Some(dark_mode);
        self.generation = Some(s.content_generation);
        self.conversation_key = Some(s.selected_conversation.clone());
        self.language = Some(language);
        self.today = Some(today);
        self.my_name = s.my_username.clone();
        self.multi_person = s
            .selected_conversation
            .as_deref()
            .is_none_or(|c| c.starts_with('#'));
        self.private_peer_display = s
            .selected_conversation
            .as_deref()
            .filter(|c| !c.starts_with('#'))
            .map(|user| s.peer_display_name(user));

        let messages = s.get_conversation_messages();

        // Noms d'affichage par auteur, résolus une fois.
        let mut authors: Vec<String> = messages.iter().map(|m| m.from.clone()).collect();
        authors.sort();
        authors.dedup();
        let display_names: HashMap<&str, String> = authors
            .iter()
            .map(|a| (a.as_str(), s.peer_display_name(a)))
            .collect();

        // Markdown memoïsé : purge des messages disparus, parse des nouveaux.
        let hashes: Vec<u64> = messages.iter().map(|m| AppState::message_hash(m)).collect();
        let live: HashSet<u64> = hashes.iter().copied().collect();
        self.markdown.retain(|h, _| live.contains(h));

        let mut image_media_ids: Vec<String> = Vec::new();
        let mut gif_urls: HashSet<String> = HashSet::new();
        let mut rows: Vec<ChatRow> = Vec::with_capacity(messages.len());
        let mut last_from: Option<&str> = None;
        let mut last_epoch: Option<u64> = None;
        let mut last_day: Option<NaiveDate> = None;

        for (msg, &hash) in messages.iter().zip(&hashes) {
            let day = message_day(msg);
            let day_changed = match (day, last_day) {
                (Some(d), Some(prev)) => d != prev,
                (Some(_), None) => last_from.is_some(),
                _ => false,
            };
            let day_label = day.map(|d| day_divider_label(d, today, language));
            let day_divider = if day_changed || last_day.is_none() {
                day_label.clone()
            } else {
                None
            };

            // Une réponse ouvre toujours un nouvel en-tête, comme Discord.
            let starts_group = msg.reply_to.is_some()
                || starts_new_group(
                    last_from,
                    last_epoch,
                    &msg.from,
                    msg.timestamp_epoch,
                    day_changed,
                );
            let is_me = msg.from == self.my_name;
            let name_color = if is_me {
                palette.success
            } else if self.multi_person {
                peer_color_for(&msg.from, dark_mode)
            } else {
                palette.receipt_read
            };
            let reply = msg.reply_to.map(|reply_hash| {
                let resolved = s.find_message_by_hash(reply_hash).cloned();
                let author = resolved
                    .as_ref()
                    .map(|m| {
                        display_names
                            .get(m.from.as_str())
                            .cloned()
                            .unwrap_or_else(|| s.peer_display_name(&m.from))
                    })
                    .unwrap_or_default();
                let author_color = resolved
                    .as_ref()
                    .map(|m| {
                        if m.from == self.my_name {
                            palette.success
                        } else if self.multi_person {
                            peer_color_for(&m.from, dark_mode)
                        } else {
                            palette.receipt_read
                        }
                    })
                    .unwrap_or(egui::Color32::GRAY);
                if let Some(media) = resolved.as_ref().and_then(|m| m.media.as_ref()) {
                    if media.kind == crate::message::MediaKind::Image
                        && !image_media_ids.contains(&media.id)
                    {
                        image_media_ids.push(media.id.clone());
                    }
                }
                ReplyInfo {
                    resolved,
                    author,
                    author_color,
                }
            });

            if let Some(media) = &msg.media {
                if media.kind == crate::message::MediaKind::Image
                    && !image_media_ids.contains(&media.id)
                {
                    image_media_ids.push(media.id.clone());
                }
                if media.kind == crate::message::MediaKind::Gif {
                    if let Some(url) = &media.url {
                        gif_urls.insert(url.clone());
                    }
                }
            }

            let markdown = self
                .markdown
                .entry(hash)
                .or_insert_with(|| Arc::new(parse_message(&msg.content, emoji_map)))
                .clone();

            let display_name = display_names
                .get(msg.from.as_str())
                .cloned()
                .unwrap_or_else(|| msg.from.clone());

            last_from = Some(msg.from.as_str());
            last_epoch = msg.timestamp_epoch;
            if day.is_some() {
                last_day = day;
            }

            rows.push(ChatRow {
                msg: (*msg).clone(),
                hash,
                day_divider,
                day_label,
                starts_group,
                header_time: header_time(msg),
                name_color,
                receipt: None,
                receipt_detail: None,
                reply,
                reactions: s.reactions_for(hash).to_vec(),
                display_name,
                markdown,
                collapse: collapse_info(&msg.content, emoji_map),
            });
        }

        // Accusés portés par l'en-tête de chaque groupe de messages, calculés
        // sur le DERNIER message du groupe (4 messages d'affilée → un seul
        // indicateur, à jour sur le dernier). En 1-à-1 : coches sur nos
        // messages. En salon/« Tous » : détail nominatif « … » sur tous les
        // messages (les coches n'ont pas de sens à plusieurs, chacun pouvant
        // avoir reçu ou lu indépendamment).
        let mut start = 0;
        while start < rows.len() {
            let mut end = start + 1;
            while end < rows.len() && !rows[end].starts_group {
                end += 1;
            }
            let last_hash = rows[end - 1].hash;
            if rows[start].starts_group {
                if self.multi_person {
                    rows[start].receipt_detail =
                        Some(s.receipt_detail(last_hash, &rows[end - 1].msg));
                } else if rows[start].msg.from == self.my_name {
                    rows[start].receipt = Some((
                        !s.is_message_pending(last_hash),
                        s.get_read_count(last_hash) > 0,
                        s.is_message_failed(last_hash),
                    ));
                }
            }
            start = end;
        }

        self.rows = Arc::new(rows);
        self.authors = authors;
        self.image_media_ids = image_media_ids;
        self.gif_urls = gif_urls;
        Some(conv_changed)
    }
}

/// Cache de la barre latérale et de la barre de saisie.
#[derive(Default)]
pub(crate) struct SidebarCache {
    generation: Option<u64>,
    presence_generation: Option<u64>,
    conversation_key: Option<Option<String>>,
    pub(crate) peers: Arc<Vec<Peer>>,
    /// Compteurs non-lus, parallèles à `peers`.
    pub(crate) unread: Vec<usize>,
    /// Noms d'affichage (alias ou username), parallèles à `peers`.
    pub(crate) display_names: Vec<String>,
    /// Épinglé en tête de liste ? parallèle à `peers`.
    pub(crate) peer_pinned: Vec<bool>,
    pub(crate) groups: Arc<Vec<Group>>,
    /// Compteurs non-lus des salons, parallèles à `groups`.
    pub(crate) group_unread: Vec<usize>,
    /// Épinglé en tête de liste ? parallèle à `groups`.
    pub(crate) group_pinned: Vec<bool>,
    pub(crate) typing: Vec<String>,
    pub(crate) selected_conversation: Option<String>,
    pub(crate) my_username: String,
    /// Le pair de la conversation 1-à-1 sélectionnée est-il en ligne
    /// (toujours vrai pour « Tous » et les groupes).
    pub(crate) selected_peer_online: bool,
    pub(crate) selected_peer_addr: Option<SocketAddr>,
}

impl SidebarCache {
    pub(crate) fn refresh(&mut self, s: &AppState) {
        if self.generation == Some(s.content_generation)
            && self.presence_generation == Some(s.presence_generation)
            && self.conversation_key.as_ref() == Some(&s.selected_conversation)
        {
            return;
        }
        self.generation = Some(s.content_generation);
        self.presence_generation = Some(s.presence_generation);
        self.conversation_key = Some(s.selected_conversation.clone());
        // Épinglés en tête, tri stable (ordre inchangé au sein de chaque
        // groupe pinné/non-pinné).
        let mut peer_order: Vec<usize> = (0..s.peers.len()).collect();
        peer_order.sort_by_key(|&i| !s.is_pinned(&s.peers[i].username));
        self.peers = Arc::new(peer_order.iter().map(|&i| s.peers[i].clone()).collect());
        self.unread = peer_order
            .iter()
            .map(|&i| s.unread_count(&s.peers[i].username))
            .collect();
        self.display_names = peer_order
            .iter()
            .map(|&i| s.peer_display_name(&s.peers[i].username))
            .collect();
        self.peer_pinned = peer_order
            .iter()
            .map(|&i| s.is_pinned(&s.peers[i].username))
            .collect();

        let mut group_order: Vec<usize> = (0..s.groups.len()).collect();
        group_order.sort_by_key(|&i| !s.is_pinned(&AppState::group_conv_key(&s.groups[i].id)));
        self.groups = Arc::new(group_order.iter().map(|&i| s.groups[i].clone()).collect());
        self.group_unread = group_order
            .iter()
            .map(|&i| s.unread_count(&AppState::group_conv_key(&s.groups[i].id)))
            .collect();
        self.group_pinned = group_order
            .iter()
            .map(|&i| s.is_pinned(&AppState::group_conv_key(&s.groups[i].id)))
            .collect();
        self.typing = s.typing_users_list();
        self.selected_conversation = s.selected_conversation.clone();
        self.my_username = s.my_username.clone();
        self.selected_peer_online = match &s.selected_conversation {
            None => true,
            Some(conv) if conv.starts_with('#') => true,
            Some(u) => s.is_peer_online(u),
        };
        self.selected_peer_addr = s.selected_peer_addr();
    }
}

#[cfg(test)]
#[path = "../tests/test_ui_snapshot.rs"]
mod tests;
