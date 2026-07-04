//! Caches de données dérivées pour le rendu, invalidés par le compteur de
//! génération de [`AppState`] : une frame ne re-dérive rien tant que l'état
//! n'a pas changé (pas de clone de conversation, pas de re-parse markdown,
//! pas de re-hash, pas de verrou long).

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use chrono::{Local, NaiveDate};
use eframe::egui;

use crate::app::{AppState, Peer};
use crate::message::{ChatMessage, Group, ReactionEntry};

use super::chat_panel::{
    day_divider_label, header_time, message_day, peer_color, starts_new_group, OWN_NAME_COLOR,
    PEER_NAME_COLOR,
};
use super::markdown::{parse_message, ParsedMarkdown};
use super::UiLanguage;

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
    /// Ouvre un groupe façon Discord (avatar + nom + heure).
    pub(crate) starts_group: bool,
    pub(crate) header_time: String,
    pub(crate) name_color: egui::Color32,
    /// Pour nos messages : (livré, lu).
    pub(crate) receipt: Option<(bool, bool)>,
    pub(crate) reply: Option<ReplyInfo>,
    pub(crate) reactions: Vec<ReactionEntry>,
    pub(crate) display_name: String,
    pub(crate) markdown: Arc<ParsedMarkdown>,
}

/// Cache du fil de la conversation sélectionnée.
#[derive(Default)]
pub(crate) struct ChatCache {
    generation: Option<u64>,
    conversation_key: Option<Option<String>>,
    language: Option<UiLanguage>,
    today: Option<NaiveDate>,
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
    ) -> Option<bool> {
        let today = Local::now().date_naive();
        let conv_changed = self.conversation_key.as_ref() != Some(&s.selected_conversation);
        if !conv_changed
            && self.generation == Some(s.content_generation)
            && self.language == Some(language)
            && self.today == Some(today)
        {
            return None;
        }
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
            let day_divider = day.filter(|_| day_changed || last_day.is_none()).map(|d| {
                day_divider_label(d, today, language)
            });

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
                OWN_NAME_COLOR
            } else if self.multi_person {
                peer_color(&msg.from)
            } else {
                PEER_NAME_COLOR
            };
            let receipt =
                is_me.then(|| (!s.is_message_pending(hash), s.get_read_count(hash) > 0));

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
                            OWN_NAME_COLOR
                        } else if self.multi_person {
                            peer_color(&m.from)
                        } else {
                            PEER_NAME_COLOR
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
                starts_group,
                header_time: header_time(msg),
                name_color,
                receipt,
                reply,
                reactions: s.reactions_for(hash).to_vec(),
                display_name,
                markdown,
            });
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
    pub(crate) groups: Arc<Vec<Group>>,
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
        self.peers = Arc::new(s.peers.clone());
        self.unread = s
            .peers
            .iter()
            .map(|p| s.unread_count(&p.username))
            .collect();
        self.display_names = s
            .peers
            .iter()
            .map(|p| s.peer_display_name(&p.username))
            .collect();
        self.groups = Arc::new(s.groups.clone());
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
