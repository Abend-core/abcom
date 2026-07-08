use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{
    AppEvent, AvatarRequest, MediaAttachment, MediaProgress, MediaSendJob, MediaStreamOffer,
    MessageAckRequest, ReactionRequest, ReadReceipt, ReadReceiptRequest, SendGroupRequest,
    SendRequest, TypingRequest,
};

mod avatar;
mod chat_panel;
pub mod composer;
mod emoji_picker;
mod events;
mod gif_picker;
mod group_modal;
mod input_bar;
mod markdown;
mod media;
mod reactions;
mod settings;
mod sidebar;
mod snapshot;
mod sound;
pub(crate) mod tray;

/// Nombre de messages affichés au départ et pas de chargement du fenêtrage
/// façon Discord (le fil charge 100 messages de plus en remontant).
pub(crate) const CHAT_WINDOW_STEP: usize = 100;

/// Emojis de réaction par défaut proposés avant tout historique d'usage,
/// façon Discord.
const DEFAULT_RECENT_EMOJIS: [&str; 6] = ["👍", "❤️", "😂", "😮", "😢", "🙏"];

/// Aperçu figé du message ciblé par une réponse en cours de composition.
/// Capturé au clic sur « répondre » pour ne pas re-verrouiller/rechercher
/// l'état à chaque frame pendant que le composeur est affiché.
pub(crate) struct ReplyTarget {
    pub(crate) message_hash: u64,
    pub(crate) author: String,
    pub(crate) content_snippet: String,
    pub(crate) media_thumb: Option<MediaAttachment>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiLanguage {
    French,
    English,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemePreference {
    System,
    Light,
    Dark,
}

/// Onglet actif de la fenêtre Paramètres.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    Profile,
    General,
    Credits,
    License,
}

/// Onglet actif du sélecteur de contenu Klipy (GIF / Mèmes / Stickers).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum GifPickerTab {
    #[default]
    Gif,
    Meme,
    Sticker,
}

/// État de l'application UI
pub(crate) struct AbcomApp {
    pub(crate) state: Arc<Mutex<AppState>>,
    /// Empreinte de notre clé publique (identité Noise), affichée dans les
    /// Paramètres pour vérification hors-bande entre utilisateurs.
    pub(crate) identity_fingerprint: String,
    /// Une passphrase de salon est active (handshake XXpsk3).
    pub(crate) psk_active: bool,
    /// Icône résidente (barre de menus / zone de notification). `None` si le
    /// système n'a pas de tray : la croix quitte alors comme avant.
    pub(crate) tray: Option<tray::Tray>,
    /// La création du tray a échoué : ne pas réessayer à chaque frame.
    pub(crate) tray_init_failed: bool,
    /// Fenêtre repliée dans le tray : aucun rendu, aucun repaint programmé,
    /// les événements réseau mettent l'état à jour et notifient nativement.
    pub(crate) window_hidden: bool,
    /// Fermeture réelle demandée (menu tray « Quitter ») : laisse passer le
    /// close_requested au lieu de cacher.
    pub(crate) really_quit: bool,
    /// Les notifications natives montrent un aperçu du message (persisté).
    pub(crate) notif_preview: bool,
    /// Lancement au démarrage de session activé (persisté + état système).
    pub(crate) autostart_enabled: bool,
    /// Résultat du décodage des PNG emoji, effectué dans un thread au
    /// démarrage : le premier frame n'attend plus les 323 décodages.
    pub(crate) emoji_decode_rx: Option<std::sync::mpsc::Receiver<Vec<(String, egui::ColorImage)>>>,
    pub(crate) event_rx: mpsc::Receiver<AppEvent>,
    pub(crate) send_tx: mpsc::Sender<SendRequest>,
    pub(crate) send_group_tx: mpsc::Sender<SendGroupRequest>,
    pub(crate) send_typing_tx: mpsc::Sender<TypingRequest>,
    pub(crate) send_read_receipt_tx: mpsc::Sender<ReadReceiptRequest>,
    pub(crate) send_ack_tx: mpsc::Sender<MessageAckRequest>,
    pub(crate) send_avatar_tx: mpsc::Sender<AvatarRequest>,
    pub(crate) send_reaction_tx: mpsc::Sender<ReactionRequest>,
    pub(crate) send_media_tx: mpsc::Sender<MediaSendJob>,
    pub(crate) input: String,
    pub(crate) input_cursor_char: usize,
    pub(crate) input_selection_anchor: Option<usize>,
    pub(crate) input_has_focus: bool,
    pub(crate) input_scroll_lines: f32,
    pub(crate) show_attachment_menu: bool,
    pub(crate) show_emoji_picker: bool,
    /// Sélecteur de contenu Klipy ouvert (GIF, Mèmes, Stickers).
    pub(crate) show_gif_picker: bool,
    /// Onglet actif du sélecteur Klipy.
    pub(crate) gif_picker_tab: GifPickerTab,
    /// Texte courant de la barre de recherche du sélecteur.
    pub(crate) gif_query: String,
    /// Feed GIF — tendances et recherche Klipy /gifs/*.
    pub(crate) gif_feed: crate::klipy::GifFeed,
    /// Feed Mèmes — tendances et recherche Klipy /static-memes/*.
    pub(crate) meme_feed: crate::klipy::GifFeed,
    /// Feed Stickers — tendances et recherche Klipy /stickers/*.
    pub(crate) sticker_feed: crate::klipy::GifFeed,
    /// Dernière frappe dans la recherche (anti-rebond avant requête).
    pub(crate) gif_last_input: std::time::Instant,
    pub(crate) show_participants: bool,
    pub(crate) enable_sound_notifications: bool,
    pub(crate) last_notification: Option<String>,
    pub(crate) notification_time: std::time::Instant,
    pub(crate) has_unread: bool,
    pub(crate) window_focused: bool,
    pub(crate) emoji_textures: Vec<(String, egui::TextureHandle)>,
    pub(crate) emoji_textures_loaded: bool,
    pub(crate) emoji_category: usize,
    pub(crate) emoji_map: std::collections::HashMap<String, usize>,
    pub(crate) emoji_alias_to_char: std::collections::HashMap<String, String>,
    pub(crate) emoji_aliases: Vec<String>,
    pub(crate) shortcode_selected: usize,
    pub(crate) show_group_modal: bool,
    pub(crate) group_name_input: String,
    pub(crate) group_members_selected: std::collections::HashSet<String>,
    /// Salon ciblé par le modal de gestion (membres, départ…) ; None = fermé.
    pub(crate) group_manage_target: Option<String>,
    /// Action destructrice du modal de gestion en attente de confirmation.
    pub(crate) group_manage_confirm: Option<group_modal::GroupConfirmAction>,
    pub(crate) last_typing_broadcast: std::time::Instant,
    pub(crate) last_retry_time: std::time::Instant,
    pub(crate) muted_conversations: std::collections::HashSet<Option<String>>,
    /// Renommage de contact : pair ciblé par la modale (None = fermée).
    pub(crate) rename_target: Option<String>,
    pub(crate) rename_input: String,
    pub(crate) drafts: std::collections::HashMap<Option<String>, String>,
    pub(crate) pending_attachments: Vec<PathBuf>,
    /// 0 = none, 1 = pick files, 2 = pick folder (deferred to next frame to avoid AppKit conflict)
    pub(crate) pending_picker: u8,
    pub(crate) ui_language: UiLanguage,
    pub(crate) theme_preference: ThemePreference,
    pub(crate) system_dark_mode: Option<bool>,
    pub(crate) show_settings: bool,
    pub(crate) settings_tab: SettingsTab,
    /// Textures d'avatars, indexées par nom d'utilisateur (cache de rendu).
    pub(crate) avatar_textures: std::collections::HashMap<String, egui::TextureHandle>,
    /// Pairs auxquels notre avatar a déjà été envoyé (évite les répétitions).
    pub(crate) avatar_sent_to: std::collections::HashSet<String>,
    /// Sélection d'image de profil différée (sélecteur natif, voir `update`).
    pub(crate) pending_avatar_pick: bool,
    /// Textures des médias image, indexées par identifiant (None = échec/non-image).
    pub(crate) media_textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Identifiant du média affiché en grand dans la visionneuse (None = fermée).
    pub(crate) media_viewer: Option<String>,
    /// Réception des offres de médias volumineux (> 1 Go) à accepter/refuser.
    pub(crate) media_offer_rx: mpsc::Receiver<MediaStreamOffer>,
    /// Offres de médias volumineux en attente de décision (bandeau).
    pub(crate) pending_media_offers: Vec<MediaStreamOffer>,
    /// Progression des transferts média en cours, par identifiant.
    pub(crate) media_progress: std::collections::HashMap<String, MediaProgress>,
    /// Ligne dont la barre d'actions au survol est affichée : (index absolu
    /// dans le fil, hash du message). L'index désambiguïse les messages au
    /// hash identique (anciens messages sans nonce) : une seule barre à la fois.
    pub(crate) hover_toolbar_target: Option<(usize, u64)>,
    /// Message ciblé par le picker de réaction ouvert (None = fermé), avec le
    /// rectangle d'ancrage du bouton "+" pour positionner la popup.
    pub(crate) reaction_picker_open: Option<(u64, egui::Rect)>,
    /// Emojis récemment utilisés en réaction (MRU, la plus récente en tête).
    pub(crate) recent_reaction_emojis: Vec<String>,
    /// Message ciblé par une réponse en cours de composition (None = aucune).
    pub(crate) replying_to: Option<ReplyTarget>,
    /// Message vers lequel faire défiler le fil au prochain rendu (clic sur
    /// une citation de réponse, façon Discord).
    pub(crate) scroll_to_message: Option<u64>,
    /// Message brièvement surligné après un saut (flash qui s'estompe).
    pub(crate) highlight_message: Option<(u64, std::time::Instant)>,
    /// Messages très longs dépliés par l'utilisateur (« Afficher la suite »),
    /// par hash — les autres restent repliés en aperçu.
    pub(crate) expanded_messages: std::collections::HashSet<u64>,
    /// Des pairs sont en train d'écrire (instantané mis à jour par
    /// `process_events`) : impose un repaint de repli court pour faire
    /// expirer l'indicateur même sans nouvel événement réseau.
    pub(crate) typing_active: bool,
    /// Dernier mode sombre effectivement appliqué à egui (évite de
    /// reconstruire les `Visuals` à chaque frame).
    pub(crate) applied_dark_mode: Option<bool>,
    /// Cache dérivé du fil (lignes pré-calculées, markdown memoïsé).
    pub(crate) chat_cache: snapshot::ChatCache,
    /// Cache dérivé de la barre latérale et de la barre de saisie.
    pub(crate) sidebar_cache: snapshot::SidebarCache,
    /// Fenêtrage du fil : nombre de messages rendus (les plus récents).
    pub(crate) chat_visible_count: usize,
    /// Hauteur de contenu avant extension de la fenêtre : sert à compenser
    /// l'offset de scroll à la frame suivante (pas de saut visuel).
    pub(crate) chat_prepend_fix: Option<f32>,
    /// Une page d'historique plus ancienne est en cours de chargement
    /// (évite les demandes répétées pendant le vol de la requête).
    pub(crate) loading_older: bool,
    /// Le picker GIF était ouvert à la frame précédente (détection de la
    /// fermeture pour libérer les aperçus du cache d'images egui).
    pub(crate) gif_picker_was_open: bool,
    /// URLs des GIFs actuellement dans le fil rendu : celles qui en sortent
    /// (changement de conversation, drain) sont retirées du cache d'images.
    pub(crate) known_gif_urls: std::collections::HashSet<String>,
    /// Ordre d'accès des textures médias (éviction LRU, cf. `media_texture`).
    pub(crate) media_texture_lru: Vec<String>,
    /// Texture pleine résolution de la visionneuse, libérée à sa fermeture
    /// (le fil n'affiche que des textures réduites).
    pub(crate) viewer_texture: Option<(String, egui::TextureHandle)>,
}

impl AbcomApp {
    // Câblage des canaux mpsc indépendants vers les tâches réseau/transfert.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        state: Arc<Mutex<AppState>>,
        identity_fingerprint: String,
        psk_active: bool,
        event_rx: mpsc::Receiver<AppEvent>,
        send_tx: mpsc::Sender<SendRequest>,
        send_group_tx: mpsc::Sender<SendGroupRequest>,
        send_typing_tx: mpsc::Sender<TypingRequest>,
        send_read_receipt_tx: mpsc::Sender<ReadReceiptRequest>,
        send_ack_tx: mpsc::Sender<MessageAckRequest>,
        send_avatar_tx: mpsc::Sender<AvatarRequest>,
        send_reaction_tx: mpsc::Sender<ReactionRequest>,
        send_media_tx: mpsc::Sender<MediaSendJob>,
        media_offer_rx: mpsc::Receiver<MediaStreamOffer>,
    ) -> Self {
        // Décodage des emojis en arrière-plan dès la création : les textures
        // seront créées (rapide) quand le résultat arrive, sans geler l'UI.
        let emoji_decode_rx = Some(spawn_emoji_decoder());

        // Préférences persistées (table kv).
        let (notif_preview, autostart_enabled) = {
            let s = state.lock().unwrap();
            (
                s.pref_bool("notif_preview", true),
                s.pref_bool("autostart", false),
            )
        };

        Self {
            state,
            identity_fingerprint,
            psk_active,
            emoji_decode_rx,
            event_rx,
            send_tx,
            send_group_tx,
            send_typing_tx,
            send_read_receipt_tx,
            send_ack_tx,
            send_avatar_tx,
            send_reaction_tx,
            send_media_tx,
            media_offer_rx,
            pending_media_offers: Vec::new(),
            media_progress: std::collections::HashMap::new(),
            input: String::new(),
            input_cursor_char: 0,
            input_selection_anchor: None,
            input_has_focus: false,
            input_scroll_lines: 0.0,
            show_attachment_menu: false,
            show_emoji_picker: false,
            show_gif_picker: false,
            gif_picker_tab: GifPickerTab::Gif,
            gif_query: String::new(),
            gif_feed: crate::klipy::GifFeed::new(crate::klipy::ContentKind::Gif),
            meme_feed: crate::klipy::GifFeed::new(crate::klipy::ContentKind::Meme),
            sticker_feed: crate::klipy::GifFeed::new(crate::klipy::ContentKind::Sticker),
            gif_last_input: std::time::Instant::now(),
            show_participants: false,
            enable_sound_notifications: true,
            last_notification: None,
            notification_time: std::time::Instant::now(),
            has_unread: false,
            window_focused: true,
            emoji_textures: Vec::new(),
            emoji_textures_loaded: false,
            emoji_category: 0,
            emoji_map: std::collections::HashMap::new(),
            emoji_alias_to_char: std::collections::HashMap::new(),
            emoji_aliases: Vec::new(),
            shortcode_selected: 0,
            show_group_modal: false,
            group_name_input: String::new(),
            group_members_selected: std::collections::HashSet::new(),
            group_manage_target: None,
            group_manage_confirm: None,
            last_typing_broadcast: std::time::Instant::now(),
            last_retry_time: std::time::Instant::now(),
            muted_conversations: std::collections::HashSet::new(),
            rename_target: None,
            rename_input: String::new(),
            drafts: std::collections::HashMap::new(),
            pending_attachments: Vec::new(),
            pending_picker: 0,
            ui_language: UiLanguage::French,
            theme_preference: ThemePreference::System,
            system_dark_mode: None,
            show_settings: false,
            settings_tab: SettingsTab::General,
            avatar_textures: std::collections::HashMap::new(),
            avatar_sent_to: std::collections::HashSet::new(),
            pending_avatar_pick: false,
            media_textures: std::collections::HashMap::new(),
            media_viewer: None,
            hover_toolbar_target: None,
            reaction_picker_open: None,
            recent_reaction_emojis: DEFAULT_RECENT_EMOJIS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            replying_to: None,
            scroll_to_message: None,
            highlight_message: None,
            expanded_messages: std::collections::HashSet::new(),
            typing_active: false,
            applied_dark_mode: None,
            tray: None,
            tray_init_failed: false,
            window_hidden: false,
            really_quit: false,
            notif_preview,
            autostart_enabled,
            chat_cache: snapshot::ChatCache::default(),
            sidebar_cache: snapshot::SidebarCache::default(),
            chat_visible_count: CHAT_WINDOW_STEP,
            chat_prepend_fix: None,
            loading_older: false,
            gif_picker_was_open: false,
            known_gif_urls: std::collections::HashSet::new(),
            media_texture_lru: Vec::new(),
            viewer_texture: None,
        }
    }

    pub(crate) fn tr(&self, french: &'static str, english: &'static str) -> &'static str {
        match self.ui_language {
            UiLanguage::French => french,
            UiLanguage::English => english,
        }
    }

    /// Sauvegarde le texte courant dans les drafts pour la conversation active
    pub(crate) fn save_draft(&mut self) {
        let selected_conv = self.state.lock().unwrap().selected_conversation.clone();
        self.drafts.insert(selected_conv, self.input.clone());
    }

    /// Charge le texte pour une conversation donnée et met à jour l'input
    pub(crate) fn load_draft(&mut self, conversation: Option<String>) {
        let draft = self.drafts.get(&conversation).cloned().unwrap_or_default();
        self.input = draft;
        self.input_cursor_char = 0;
        self.input_selection_anchor = None;
        self.input_has_focus = false;
        self.input_scroll_lines = 0.0;
    }

    /// Change vers une nouvelle conversation, sauvegardant le draft actuel et chargeant celui de la nouvelle
    pub(crate) fn switch_conversation(&mut self, new_conversation: Option<String>) {
        self.save_draft();
        self.state.lock().unwrap().selected_conversation = new_conversation.clone();
        self.load_draft(new_conversation.clone());

        // ReadReceipts différés pour tous les messages reçus dans cette
        // conversation (privée, salon #… ou « Tous »).
        self.send_read_receipts_for_conversation(new_conversation);
    }

    /// Envoie un ReadReceipt pour chaque message reçu d'un autre pair dans la
    /// conversation donnée : pair (privé), `#nom` (salon) ou `None` (« Tous »).
    /// En salon/« Tous », l'accusé est diffusé à tous les membres en ligne
    /// pour que chacun voie le même détail « … » reçu/lu.
    pub(crate) fn send_read_receipts_for_conversation(&mut self, conv: Option<String>) {
        let s = self.state.lock().unwrap();
        let my_name = s.my_username.clone();
        let now = chrono::Local::now().format("%H:%M").to_string();

        let mut receipts: Vec<ReadReceiptRequest> = Vec::new();
        for m in s.messages.iter().filter(|m| m.from != my_name) {
            let in_conv = match (conv.as_deref(), m.to_user.as_deref()) {
                (None, None) => true,
                (Some(c), Some(t)) if c.starts_with('#') => t == c,
                (Some(c), Some(t)) => m.from == c && t == my_name,
                _ => false,
            };
            if !in_conv {
                continue;
            }
            let hash = crate::app::AppState::message_hash(m);
            for addr in s.receipt_recipients(m) {
                receipts.push(ReadReceiptRequest {
                    to_addr: addr,
                    receipt: ReadReceipt {
                        from: my_name.clone(),
                        to: m.from.clone(),
                        message_hash: hash,
                        timestamp: now.clone(),
                    },
                });
            }
        }
        drop(s);

        for req in receipts {
            let _ = self.send_read_receipt_tx.try_send(req);
        }
    }
}

impl AbcomApp {
    /// Replie la fenêtre dans le tray : rendu stoppé, textures libérées
    /// (elles seront rechargées paresseusement à la réouverture). Sur macOS,
    /// l'application quitte aussi le Dock (politique Accessory) : elle ne
    /// vit plus que dans la barre de menus.
    pub(crate) fn hide_to_tray(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        set_dock_visible(false);
        self.window_hidden = true;
        self.window_focused = false;

        // Purge mémoire : textures GPU et caches d'images.
        self.media_textures.clear();
        self.media_texture_lru.clear();
        self.avatar_textures.clear();
        self.viewer_texture = None;
        self.media_viewer = None;
        for url in &self.known_gif_urls {
            ctx.forget_image(url);
        }
        self.forget_gif_previews(ctx);
        // Emojis : libérés aussi, re-décodés en arrière-plan au retour.
        self.emoji_textures.clear();
        self.emoji_map.clear();
        self.emoji_textures_loaded = false;
        self.emoji_decode_rx = None;
        self.chat_cache.invalidate();
    }

    /// Restaure la fenêtre depuis le tray et resynchronise l'affichage
    /// (l'état est déjà à jour : seuls les caches de rendu se reconstruisent).
    pub(crate) fn show_from_tray(&mut self, ctx: &egui::Context) {
        if !self.window_hidden {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            return;
        }
        self.window_hidden = false;
        set_dock_visible(true);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.emoji_decode_rx = Some(spawn_emoji_decoder());
        self.chat_cache.invalidate();
        ctx.request_repaint();
    }

    /// Notification système native (fenêtre cachée/minimisée). Envoyée d'un
    /// thread détaché : l'appel peut bloquer selon l'OS.
    pub(crate) fn notify_native(summary: String, body: String) {
        std::thread::Builder::new()
            .name("abcom-notify".into())
            .spawn(move || {
                let _ = notify_rust::Notification::new()
                    .appname("Abcom")
                    .summary(&summary)
                    .body(&body)
                    .show();
            })
            .ok();
    }

    /// Corps de notification pour un message entrant, selon la préférence
    /// « aperçu » (persistée).
    pub(crate) fn native_body_for(&self, content: &str) -> String {
        if self.notif_preview {
            media::elide(content, 120)
        } else {
            self.tr("Nouveau message", "New message").to_string()
        }
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Icône résidente, créée paresseusement (macOS impose le thread
        // principal avec l'event loop démarrée — c'est le cas ici).
        if self.tray.is_none() && !self.tray_init_failed {
            self.tray = tray::Tray::new(
                self.tr("Ouvrir Abcom", "Open Abcom"),
                self.tr("Quitter", "Quit"),
            );
            if self.tray.is_none() {
                self.tray_init_failed = true;
                eprintln!("[tray] Icône résidente indisponible : la croix quittera l'application");
            }
        }

        // Actions tray (les callbacks ont déjà réveillé l'UI).
        if let Some(t) = &self.tray {
            for action in t.poll() {
                match action {
                    tray::TrayAction::Open => self.show_from_tray(ctx),
                    tray::TrayAction::Quit => {
                        self.really_quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        // Croix / Cmd-W : cacher au lieu de quitter — uniquement si un tray
        // existe pour rouvrir (sinon comportement historique).
        if ctx.input(|i| i.viewport().close_requested())
            && !self.really_quit
            && self.tray.is_some()
            && !self.window_hidden
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_to_tray(ctx);
        }

        self.window_focused = !self.window_hidden && ctx.input(|i| i.focused);
        self.process_events();
        self.process_media_offers();
        self.periodic_tasks();

        // Badge non-lus sur l'icône résidente.
        let unread = self.has_unread;
        if let Some(t) = &mut self.tray {
            t.set_unread(unread);
        }

        // Cachée ou minimisée : l'état et SQLite sont à jour, les
        // notifications natives sont parties — aucun rendu, aucun repaint
        // programmé (CPU/GPU ~0). Le prochain réveil viendra du réseau, du
        // tray ou de la restauration de la fenêtre.
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if self.window_hidden || minimized {
            return;
        }

        self.apply_theme_preference(ctx);
        self.lazy_load_emoji(ctx);

        // Rafraîchit les caches dérivés si l'état a changé (génération) —
        // sinon la frame se rend sans reprendre le verrou ni rien re-dériver.
        {
            let s = self.state.lock().unwrap();
            self.sidebar_cache.refresh(&s);
            let rebuilt = self
                .chat_cache
                .refresh(&s, self.ui_language, &self.emoji_map);
            drop(s);
            if let Some(conv_changed) = rebuilt {
                if conv_changed {
                    self.chat_visible_count = CHAT_WINDOW_STEP;
                    self.chat_prepend_fix = None;
                }
                // Les GIFs sortis du fil (changement de conversation ou
                // expiration du ring-buffer) libèrent leurs frames décodées.
                for url in self.known_gif_urls.difference(&self.chat_cache.gif_urls) {
                    ctx.forget_image(url);
                }
                self.known_gif_urls = self.chat_cache.gif_urls.clone();
            }
        }

        // Flash barre des tâches si message non lu — réinitialisé une seule fois
        // quand la fenêtre reprend le focus (pas d'envoi répété en boucle).
        if self.has_unread && self.window_focused {
            self.has_unread = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Reset,
            ));
        }

        // Handle deferred native file/folder picker (must run before egui rendering to
        // avoid conflicting with the AppKit run-loop on macOS).
        if self.pending_picker != 0 {
            let kind = self.pending_picker;
            self.pending_picker = 0;
            let (files_title, folder_title, files_added, folder_added) = (
                self.tr("Ajouter des fichiers", "Add files"),
                self.tr("Ajouter un dossier", "Add folder"),
                self.tr("Fichiers ajoutés", "Files added"),
                self.tr("Dossier ajouté", "Folder added"),
            );
            match kind {
                1 => {
                    if let Some(paths) = rfd::FileDialog::new().set_title(files_title).pick_files()
                    {
                        for p in paths {
                            if !self.pending_attachments.contains(&p) {
                                self.pending_attachments.push(p);
                            }
                        }
                        self.last_notification = Some(files_added.to_string());
                        self.notification_time = std::time::Instant::now();
                    }
                }
                2 => {
                    if let Some(path) = rfd::FileDialog::new().set_title(folder_title).pick_folder()
                    {
                        if !self.pending_attachments.contains(&path) {
                            self.pending_attachments.push(path);
                        }
                        self.last_notification = Some(folder_added.to_string());
                        self.notification_time = std::time::Instant::now();
                    }
                }
                _ => {}
            }
        }

        // Sélection de l'image de profil (différée comme les autres sélecteurs
        // natifs pour éviter un conflit avec la run-loop AppKit sur macOS).
        if self.pending_avatar_pick {
            self.pending_avatar_pick = false;
            let (pick_title, error_msg) = (
                self.tr("Choisir une image de profil", "Choose a profile picture"),
                self.tr("Image de profil invalide", "Invalid profile picture"),
            );
            if let Some(path) = rfd::FileDialog::new()
                .set_title(pick_title)
                .add_filter("Images", &["png", "jpg", "jpeg", "svg"])
                .pick_file()
            {
                match avatar::load_normalized_avatar(&path) {
                    Ok(png) => {
                        let my_name = self.state.lock().unwrap().my_username.clone();
                        self.state.lock().unwrap().set_my_avatar(png);
                        self.avatar_textures.remove(&my_name);
                        self.broadcast_my_avatar();
                    }
                    Err(e) => {
                        eprintln!("[ui] Avatar non chargé : {}", e);
                        self.last_notification = Some(error_msg.to_string());
                        self.notification_time = std::time::Instant::now();
                    }
                }
            }
        }

        self.show_sidebar_panel(ctx);
        let (emoji_btn_clicked, gif_btn_clicked) = self.show_input_bar(ctx);
        self.show_notification(ctx);
        self.show_emoji_picker_window(ctx, emoji_btn_clicked);
        self.show_gif_picker_window(ctx, gif_btn_clicked);
        self.render_group_modal(ctx);
        self.render_group_manage_modal(ctx);
        self.show_central_panel(ctx);
        self.show_reaction_emoji_picker(ctx);
        self.render_settings(ctx);
        self.show_media_viewer(ctx);

        // Repaint de repli : les événements réseau réveillent déjà l'UI
        // (cf. notify.rs), il ne reste à couvrir que les états transitoires
        // à expiration temporelle. Au repos : une frame toutes les 5 s
        // (nettoyage des pairs inactifs), soit un CPU/GPU quasi nul.
        let fallback = if self.last_notification.is_some() || self.highlight_message.is_some() {
            Duration::from_millis(500) // expiration notification / flash de surlignage
        } else if self.typing_active {
            Duration::from_secs(1) // expiration de l'indicateur « écrit… »
        } else if !self.window_focused {
            Duration::from_secs(30) // visible en arrière-plan : quasi dormant
        } else {
            Duration::from_secs(5) // tick periodic_tasks
        };
        ctx.request_repaint_after(fallback);
    }

    /// Flush final du stockage : attend que toutes les écritures en file
    /// soient appliquées avant la fermeture.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.state.lock().unwrap().flush_storage();
    }
}

/// Nom de la famille de police en gras enregistrée dans egui (Inter Bold).
/// egui ne synthétise pas le gras : on charge une vraie police pour les noms.
pub(crate) const BOLD_FAMILY: &str = "bold";

/// Définitions de polices : on conserve les polices par défaut et on ajoute
/// Inter Bold (OFL) sous la famille [`BOLD_FAMILY`] pour les noms d'auteur.
fn build_fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "inter-bold".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-Bold.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name(BOLD_FAMILY.into()),
        vec!["inter-bold".to_owned()],
    );
    fonts
}

fn app_icon_data() -> Option<egui::IconData> {
    let data = include_bytes!("../../assets/app_icon.png");
    eprintln!("[ui] Chargement icône PNG ({} bytes)", data.len());
    match image::load_from_memory(data) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            eprintln!("[ui] Icône chargée : {}x{}", w, h);
            Some(egui::IconData {
                rgba: rgba.to_vec(),
                width: w,
                height: h,
            })
        }
        Err(err) => {
            eprintln!("[ui] Erreur icône PNG : {}", err);
            let mut rgba = vec![0u8; 32 * 32 * 4];
            for i in 0..(32 * 32) {
                rgba[i * 4] = 200;
                rgba[i * 4 + 1] = 50;
                rgba[i * 4 + 2] = 50;
                rgba[i * 4 + 3] = 255;
            }
            Some(egui::IconData {
                rgba,
                width: 32,
                height: 32,
            })
        }
    }
}

/// Point d'entrée de l'interface graphique
// Câblage des canaux mpsc indépendants transmis tels quels à `AbcomApp::new`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    state: Arc<Mutex<AppState>>,
    ui_ctx: crate::notify::UiContext,
    identity_fingerprint: String,
    psk_active: bool,
    event_rx: mpsc::Receiver<AppEvent>,
    send_tx: mpsc::Sender<SendRequest>,
    send_group_tx: mpsc::Sender<SendGroupRequest>,
    send_typing_tx: mpsc::Sender<TypingRequest>,
    send_read_receipt_tx: mpsc::Sender<ReadReceiptRequest>,
    send_ack_tx: mpsc::Sender<MessageAckRequest>,
    send_avatar_tx: mpsc::Sender<AvatarRequest>,
    send_reaction_tx: mpsc::Sender<ReactionRequest>,
    send_media_tx: mpsc::Sender<MediaSendJob>,
    media_offer_rx: mpsc::Receiver<MediaStreamOffer>,
) -> anyhow::Result<()> {
    // Handlers tray/menu globaux : chaque événement réveille l'UI via le
    // contexte partagé (fonctionne même fenêtre cachée, sans rendu).
    tray::install_event_handlers(ui_ctx.clone());

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Abcom")
        .with_inner_size([860.0, 600.0]);

    if let Some(icon) = app_icon_data() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Abcom",
        options,
        Box::new(move |cc| {
            // Publie le contexte egui vers les tâches de fond : chaque
            // événement relayé peut désormais réveiller la boucle de rendu.
            let _ = ui_ctx.set(cc.egui_ctx.clone());
            cc.egui_ctx.set_fonts(build_fonts());
            // Loaders d'images egui_extras : HTTP (récupération depuis le CDN
            // Klipy) + décodage GIF/WebP animés pour les vignettes et le fil.
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(AbcomApp::new(
                state,
                identity_fingerprint,
                psk_active,
                event_rx,
                send_tx,
                send_group_tx,
                send_typing_tx,
                send_read_receipt_tx,
                send_ack_tx,
                send_avatar_tx,
                send_reaction_tx,
                send_media_tx,
                media_offer_rx,
            )))
        }),
    )
    .map_err(|e| {
        eprintln!("Erreur GUI : {}", e);
        eprintln!("Sur WSL sans GPU, utilisez make run-windows.");
        anyhow::anyhow!("Échec GUI : {}", e)
    })?;

    Ok(())
}

/// Décode les PNG du registre d'emojis dans un thread dédié (le premier
/// frame de l'UI n'attend plus ~323 décodages d'images).
fn spawn_emoji_decoder() -> std::sync::mpsc::Receiver<Vec<(String, egui::ColorImage)>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("abcom-emoji".into())
        .spawn(move || {
            let images: Vec<(String, egui::ColorImage)> = crate::emoji_registry::EMOJI_DATA
                .iter()
                .filter_map(|(ch, bytes)| {
                    image::load_from_memory(bytes).ok().map(|img| {
                        let rgba = img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            rgba.as_raw(),
                        );
                        (ch.to_string(), color_image)
                    })
                })
                .collect();
            let _ = tx.send(images);
        })
        .ok();
    rx
}

/// macOS : montre/retire l'icône du Dock. Repliée dans la barre de menus,
/// l'application passe en politique `Accessory` (plus de Dock ni de Cmd-Tab) ;
/// à la réouverture elle redevient `Regular` et revient au premier plan.
/// Doit être appelé sur le thread principal (c'est le cas dans `update`).
#[cfg(target_os = "macos")]
fn set_dock_visible(visible: bool) {
    use objc2::ClassType;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSImage};
    use objc2_foundation::NSData;
    let Some(mtm) = objc2_foundation::MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let policy = if visible {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    app.setActivationPolicy(policy);
    if visible {
        // Revenir au premier plan après la sortie du mode Accessory.
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        // Le retour en politique Regular réinitialise l'icône du Dock à
        // l'icône générique d'exécutable : ré-applique la nôtre.
        let data = NSData::with_bytes(include_bytes!("../../assets/app_icon.png"));
        if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
            // Sûr : image valide construite ci-dessus, appel sur le thread
            // principal (garanti par le MainThreadMarker).
            unsafe { app.setApplicationIconImage(Some(&image)) };
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_dock_visible(_visible: bool) {}
