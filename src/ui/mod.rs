use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{
    AppEvent, MediaAttachment, MediaProgress, MediaSendJob, MediaStreamOffer, NetworkSendRequest,
    ReadReceipt, ReadReceiptRequest,
};
use crate::platform::tray;
use crate::util::MutexExt;

mod avatar;
mod chat_panel;
pub mod composer;
mod emoji_picker;
mod events;
mod gif_picker;
mod group_modal;
mod i18n;
mod input_bar;
mod markdown;
mod media;
mod outbound;
mod picker;
mod reactions;
mod search;
mod settings;
mod sidebar;
mod snapshot;
mod sound;
mod theme;

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

/// Canaux vers les tâches réseau : les paquets courts partagent une commande
/// typée, le streaming média reste séparé.
pub(crate) struct NetworkChannels {
    pub(crate) event_rx: mpsc::Receiver<AppEvent>,
    /// Émetteur d'événements pour le travail lourd déporté hors du thread UI
    /// (copie d'un média vers Téléchargements), qui doit rendre son verdict.
    pub(crate) event_tx: mpsc::Sender<AppEvent>,
    pub(crate) send_tx: mpsc::Sender<NetworkSendRequest>,
    pub(crate) send_media_tx: mpsc::Sender<MediaSendJob>,
}

impl NetworkChannels {
    fn try_send(&self, request: impl Into<NetworkSendRequest>) -> bool {
        match self.send_tx.try_send(request.into()) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                crate::metrics::record_packet_dropped();
                tracing::warn!("commande réseau ignorée : file pleine");
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                crate::metrics::record_packet_dropped();
                tracing::warn!("commande réseau ignorée : canal fermé");
                false
            }
        }
    }

    /// Signaux non critiques : perte acceptable, mais comptée pour le diagnostic.
    fn try_send_best_effort(&self, request: impl Into<NetworkSendRequest>) {
        match self.send_tx.try_send(request.into()) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Closed(_)) => {
                crate::metrics::record_packet_dropped();
                tracing::debug!("signal réseau abandonné : canal fermé");
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                crate::metrics::record_packet_dropped();
                tracing::debug!("signal réseau abandonné : file pleine");
            }
        }
    }
}

/// Canaux créés par le runtime et transférés en bloc à l'interface.
pub struct UiRuntimeChannels {
    pub event_rx: mpsc::Receiver<AppEvent>,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub send_tx: mpsc::Sender<NetworkSendRequest>,
    pub send_media_tx: mpsc::Sender<MediaSendJob>,
    pub media_offer_rx: mpsc::Receiver<MediaStreamOffer>,
    /// Magasin TOFU partagé : l'UI n'y touche que pour le ré-appairage explicite.
    pub trust: Arc<crate::network::secure::TrustStore>,
}

/// Textures emoji décodées à la demande, indexées comme `emoji_registry::EMOJI_DATA`.
///
/// Les 323 PNG étaient décodés et téléversés en GPU au lancement même si le
/// sélecteur n'était jamais ouvert. La mutabilité intérieure permet de garder
/// les emprunts partagés du code de rendu.
#[derive(Default)]
pub struct EmojiTextures {
    cache: std::cell::RefCell<std::collections::HashMap<usize, Option<egui::TextureHandle>>>,
}

impl EmojiTextures {
    /// Texture d'un emoji, décodée au premier affichage puis mémorisée.
    pub fn get(&self, ctx: &egui::Context, index: usize) -> Option<egui::TextureHandle> {
        if let Some(cached) = self.cache.borrow().get(&index) {
            return cached.clone();
        }
        let decoded = crate::emoji_registry::EMOJI_DATA
            .get(index)
            .and_then(|(ch, bytes)| {
                let rgba = image::load_from_memory(bytes).ok()?.to_rgba8();
                let (w, h) = rgba.dimensions();
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                Some(ctx.load_texture(format!("emoji_{ch}"), image, egui::TextureOptions::LINEAR))
            });
        self.cache.borrow_mut().insert(index, decoded.clone());
        decoded
    }

    pub fn clear(&self) {
        self.cache.borrow_mut().clear();
    }
}

/// Index de recherche emoji et état de navigation du picker.
pub(crate) struct EmojiPickerState {
    pub(crate) textures: EmojiTextures,
    pub(crate) category: usize,
    pub(crate) map: std::collections::HashMap<String, usize>,
    pub(crate) alias_to_char: std::collections::HashMap<String, String>,
    pub(crate) aliases: Vec<String>,
    /// Suggestion sélectionnée dans le menu de complétion `:shortcode`.
    pub(crate) shortcode_selected: usize,
}

/// État du sélecteur de contenu Klipy (GIF / Mèmes / Stickers) : trois feeds
/// indépendants, l'onglet actif et la recherche partagée.
pub(crate) struct GifPickerState {
    pub(crate) show: bool,
    pub(crate) tab: GifPickerTab,
    /// Texte courant de la barre de recherche du sélecteur.
    pub(crate) query: String,
    /// Feed GIF — tendances et recherche Klipy /gifs/*.
    pub(crate) feed: crate::services::klipy::GifFeed,
    /// Feed Mèmes — tendances et recherche Klipy /static-memes/*.
    pub(crate) meme_feed: crate::services::klipy::GifFeed,
    /// Feed Stickers — tendances et recherche Klipy /stickers/*.
    pub(crate) sticker_feed: crate::services::klipy::GifFeed,
    /// Dernière frappe dans la recherche (anti-rebond avant requête).
    pub(crate) last_input: std::time::Instant,
    /// Le picker était ouvert à la frame précédente (détection de la
    /// fermeture pour libérer les aperçus du cache d'images egui).
    pub(crate) was_open: bool,
}

/// État de la zone de saisie : texte courant, position du curseur/sélection,
/// brouillons par conversation et pièces jointes en attente d'envoi.
pub(crate) struct ComposerState {
    pub(crate) text: String,
    pub(crate) cursor_char: usize,
    pub(crate) selection_anchor: Option<usize>,
    pub(crate) has_focus: bool,
    pub(crate) scroll_lines: f32,
    /// Texte non envoyé par conversation, restauré au changement d'onglet.
    pub(crate) drafts: std::collections::HashMap<Option<String>, String>,
    pub(crate) pending_attachments: Vec<PathBuf>,
}

/// Recherche plein texte dans l'historique (Cmd/Ctrl+F).
#[derive(Default)]
pub(crate) struct SearchState {
    pub(crate) open: bool,
    pub(crate) query: String,
    /// Le champ doit prendre le focus au prochain rendu.
    pub(crate) focus_requested: bool,
    /// Requête déjà envoyée au stockage (évite de la relancer par frame).
    pub(crate) submitted: String,
    pub(crate) results: Vec<crate::message::ChatMessage>,
}

/// État des modales et panneaux superposés : création/gestion de salon,
/// renommage de contact, paramètres, liste des participants.
pub(crate) struct ModalsState {
    pub(crate) participants_open: bool,
    pub(crate) group_modal_open: bool,
    pub(crate) group_name_input: String,
    pub(crate) group_members_selected: std::collections::HashSet<String>,
    /// Salon ciblé par le modal de gestion (membres, départ…) ; None = fermé.
    pub(crate) group_manage_target: Option<String>,
    /// Action destructrice du modal de gestion en attente de confirmation.
    pub(crate) group_manage_confirm: Option<group_modal::GroupConfirmAction>,
    /// Renommage de contact : pair ciblé par la modale (None = fermée).
    pub(crate) rename_target: Option<String>,
    pub(crate) rename_input: String,
    pub(crate) settings_open: bool,
    pub(crate) settings_tab: SettingsTab,
    /// Pair dont la clé a changé (TOFU `Mismatch`) et clé qu'il a présentée :
    /// la modale propose de ré-épingler **celle-ci**. None = aucune alerte.
    pub(crate) key_mismatch: Option<(String, Vec<u8>)>,
}

/// État des médias du fil : caches de textures (éviction LRU), visionneuse
/// plein écran, et réception/décision des offres de transfert volumineux.
pub(crate) struct MediaState {
    /// Textures des médias image, indexées par identifiant (None = échec/non-image).
    pub(crate) textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Identifiant du média affiché en grand dans la visionneuse (None = fermée).
    pub(crate) viewer: Option<String>,
    /// Réception des offres de médias volumineux (au-delà du seuil d'accord) à accepter/refuser.
    pub(crate) offer_rx: mpsc::Receiver<MediaStreamOffer>,
    /// Offres de médias volumineux en attente de décision (bandeau).
    pub(crate) pending_offers: Vec<MediaStreamOffer>,
    /// Progression des transferts média en cours, par identifiant.
    pub(crate) progress: std::collections::HashMap<String, MediaProgress>,
    /// URLs des GIFs actuellement dans le fil rendu : celles qui en sortent
    /// (changement de conversation, drain) sont retirées du cache d'images.
    pub(crate) known_gif_urls: std::collections::HashSet<String>,
    /// Ordre d'accès des textures médias (éviction LRU, cf. `media_texture`).
    pub(crate) texture_lru: Vec<String>,
    /// Texture pleine résolution de la visionneuse, libérée à sa fermeture
    /// (le fil n'affiche que des textures réduites).
    pub(crate) viewer_texture: Option<(String, egui::TextureHandle)>,
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
    pub(crate) net: NetworkChannels,
    pub(crate) composer: ComposerState,
    pub(crate) show_attachment_menu: bool,
    pub(crate) show_emoji_picker: bool,
    pub(crate) gif_picker: GifPickerState,
    pub(crate) enable_sound_notifications: bool,
    pub(crate) last_notification: Option<String>,
    pub(crate) notification_time: std::time::Instant,
    pub(crate) has_unread: bool,
    pub(crate) window_focused: bool,
    pub(crate) emoji: EmojiPickerState,
    pub(crate) modals: ModalsState,
    pub(crate) last_typing_broadcast: std::time::Instant,
    /// Accusés prêts mais retenus jusqu'au commit du message correspondant
    /// (cf. `AppEvent::MessagesPersisted`), par hash.
    pub(crate) pending_acks: std::collections::HashMap<u64, Vec<NetworkSendRequest>>,
    pub(crate) last_retry_time: std::time::Instant,
    /// Dernier passage du GC du cache média (cf. `MEDIA_GC_INTERVAL`).
    pub(crate) last_media_gc: std::time::Instant,
    pub(crate) muted_conversations: std::collections::HashSet<Option<String>>,
    /// 0 = none, 1 = pick files, 2 = pick folder (deferred to next frame to avoid AppKit conflict)
    pub(crate) pending_picker: u8,
    pub(crate) ui_language: UiLanguage,
    /// Préférence de thème : egui suit le système et détecte ses changements
    /// en cours d'exécution, ce que notre détection au démarrage ne faisait pas.
    pub(crate) theme_preference: egui::ThemePreference,
    /// Textures d'avatars, indexées par nom d'utilisateur (cache de rendu).
    pub(crate) avatar_textures: std::collections::HashMap<String, egui::TextureHandle>,
    /// Pairs auxquels notre avatar a déjà été envoyé (évite les répétitions).
    pub(crate) avatar_sent_to: std::collections::HashSet<String>,
    /// Sélection d'image de profil différée (sélecteur natif, voir `update`).
    pub(crate) pending_avatar_pick: bool,
    /// Export de conversation demandé : sélecteur natif ouvert à la frame suivante.
    pub(crate) pending_export: bool,
    pub(crate) media: MediaState,
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
    pub(crate) search: SearchState,
    /// Magasin TOFU, pour le seul ré-appairage explicite (cf. `modals.key_mismatch`).
    pub(crate) trust: Arc<crate::network::secure::TrustStore>,
    /// Accusés déjà émis par destinataire : sans ce mémo, chaque ouverture rediffusait toute la fenêtre.
    pub(crate) read_receipts_sent:
        std::collections::HashMap<String, std::collections::HashSet<u64>>,
    /// Verdicts des sélecteurs de fichiers natifs, qui vivent hors du thread
    /// de rendu (cf. `picker`).
    pub(crate) picker_tx: std::sync::mpsc::Sender<picker::PickerOutcome>,
    picker_rx: std::sync::mpsc::Receiver<picker::PickerOutcome>,
}

impl AbcomApp {
    pub(crate) fn new(
        state: Arc<Mutex<AppState>>,
        identity_fingerprint: String,
        psk_active: bool,
        channels: UiRuntimeChannels,
    ) -> Self {
        // Index emoji construit sur le registre statique : aucun décodage ici.
        let characters: Vec<String> = crate::emoji_registry::EMOJI_DATA
            .iter()
            .map(|(ch, _)| (*ch).to_string())
            .collect();
        let emoji_map: std::collections::HashMap<String, usize> = characters
            .iter()
            .enumerate()
            .map(|(i, ch)| (ch.clone(), i))
            .collect();
        let (alias_to_char, aliases) = emoji_picker::build_emoji_shortcode_index(&characters);
        let (picker_tx, picker_rx) = std::sync::mpsc::channel();
        // Préférences persistées (table kv).
        let (notif_preview, autostart_enabled) = {
            let s = state.lock_safe();
            (
                s.pref_bool("notif_preview", true),
                s.pref_bool("autostart", false),
            )
        };

        Self {
            state,
            identity_fingerprint,
            psk_active,
            net: NetworkChannels {
                event_rx: channels.event_rx,
                event_tx: channels.event_tx,
                send_tx: channels.send_tx,
                send_media_tx: channels.send_media_tx,
            },
            media: MediaState {
                textures: std::collections::HashMap::new(),
                viewer: None,
                offer_rx: channels.media_offer_rx,
                pending_offers: Vec::new(),
                progress: std::collections::HashMap::new(),
                known_gif_urls: std::collections::HashSet::new(),
                texture_lru: Vec::new(),
                viewer_texture: None,
            },
            composer: ComposerState {
                text: String::new(),
                cursor_char: 0,
                selection_anchor: None,
                has_focus: false,
                scroll_lines: 0.0,
                drafts: std::collections::HashMap::new(),
                pending_attachments: Vec::new(),
            },
            show_attachment_menu: false,
            show_emoji_picker: false,
            gif_picker: GifPickerState {
                show: false,
                tab: GifPickerTab::Gif,
                query: String::new(),
                feed: crate::services::klipy::GifFeed::new(
                    crate::services::klipy::ContentKind::Gif,
                ),
                meme_feed: crate::services::klipy::GifFeed::new(
                    crate::services::klipy::ContentKind::Meme,
                ),
                sticker_feed: crate::services::klipy::GifFeed::new(
                    crate::services::klipy::ContentKind::Sticker,
                ),
                last_input: std::time::Instant::now(),
                was_open: false,
            },
            enable_sound_notifications: true,
            last_notification: None,
            notification_time: std::time::Instant::now(),
            has_unread: false,
            window_focused: true,
            emoji: EmojiPickerState {
                textures: EmojiTextures::default(),
                category: 0,
                map: emoji_map,
                alias_to_char,
                aliases,
                shortcode_selected: 0,
            },
            modals: ModalsState {
                participants_open: false,
                group_modal_open: false,
                group_name_input: String::new(),
                group_members_selected: std::collections::HashSet::new(),
                group_manage_target: None,
                group_manage_confirm: None,
                rename_target: None,
                rename_input: String::new(),
                settings_open: false,
                settings_tab: SettingsTab::General,
                key_mismatch: None,
            },
            last_typing_broadcast: std::time::Instant::now(),
            pending_acks: std::collections::HashMap::new(),
            last_retry_time: std::time::Instant::now(),
            last_media_gc: std::time::Instant::now(),
            muted_conversations: std::collections::HashSet::new(),
            pending_picker: 0,
            ui_language: UiLanguage::French,
            theme_preference: egui::ThemePreference::System,
            avatar_textures: std::collections::HashMap::new(),
            avatar_sent_to: std::collections::HashSet::new(),
            pending_avatar_pick: false,
            pending_export: false,
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
            search: SearchState::default(),
            trust: channels.trust,
            read_receipts_sent: std::collections::HashMap::new(),
            picker_tx,
            picker_rx,
        }
    }

    /// Libellé du catalogue dans la langue active.
    pub(crate) fn t(&self, entry: i18n::Entry) -> &'static str {
        entry.get(self.ui_language)
    }

    /// Sauvegarde le texte courant dans les drafts pour la conversation active
    pub(crate) fn save_draft(&mut self) {
        let selected_conv = self.state.lock_safe().selected_conversation.clone();
        self.composer
            .drafts
            .insert(selected_conv, self.composer.text.clone());
    }

    /// Charge le texte pour une conversation donnée et met à jour l'input
    pub(crate) fn load_draft(&mut self, conversation: Option<String>) {
        let draft = self
            .composer
            .drafts
            .get(&conversation)
            .cloned()
            .unwrap_or_default();
        self.composer.text = draft;
        self.composer.cursor_char = 0;
        self.composer.selection_anchor = None;
        self.composer.has_focus = false;
        self.composer.scroll_lines = 0.0;
    }

    /// Change vers une nouvelle conversation, sauvegardant le draft actuel et chargeant celui de la nouvelle
    pub(crate) fn switch_conversation(&mut self, new_conversation: Option<String>) {
        self.save_draft();
        self.state.lock_safe().selected_conversation = new_conversation.clone();
        self.load_draft(new_conversation.clone());

        // ReadReceipts différés pour tous les messages reçus dans cette
        // conversation (privée, salon #… ou « Tous »).
        self.send_read_receipts_for_conversation(new_conversation);
    }

    /// Applique les raccourcis globaux. Appelé avant le rendu des panneaux
    /// pour que la combinaison soit consommée avant qu'un widget la voie.
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts::SETTINGS)) {
            self.modals.settings_open = !self.modals.settings_open;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts::SEARCH)) {
            self.search.open = true;
            self.search.focus_requested = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcuts::CLOSE_OVERLAY)) {
            self.close_topmost_overlay();
        }

        let step = if ctx.input_mut(|i| i.consume_shortcut(&shortcuts::NEXT_CONVERSATION)) {
            1
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcuts::PREV_CONVERSATION)) {
            -1
        } else {
            0
        };
        if step != 0 {
            self.cycle_conversation(step);
        }
    }

    /// Ferme la surcouche visible la plus haute, dans l'ordre où l'utilisateur
    /// s'attend à les voir disparaître.
    fn close_topmost_overlay(&mut self) {
        if self.media.viewer.take().is_some() {
            return;
        }
        for open in [
            &mut self.gif_picker.show,
            &mut self.show_emoji_picker,
            &mut self.modals.settings_open,
            &mut self.modals.group_modal_open,
            &mut self.search.open,
        ] {
            if *open {
                *open = false;
                return;
            }
        }
        self.modals.rename_target = None;
        self.modals.group_manage_target = None;
    }

    /// Passe à la conversation suivante ou précédente de la barre latérale.
    fn cycle_conversation(&mut self, step: isize) {
        // Même ordre que la sidebar : « Tous », puis les pairs, puis les salons.
        let mut keys: Vec<Option<String>> = vec![None];
        keys.extend(
            self.sidebar_cache
                .peers
                .iter()
                .map(|peer| Some(peer.username.clone())),
        );
        keys.extend(
            self.sidebar_cache
                .groups
                .iter()
                .map(|group| Some(format!("#{}", group.name))),
        );
        if keys.len() < 2 {
            return;
        }

        let current = self.state.lock_safe().selected_conversation.clone();
        let index = keys.iter().position(|k| *k == current).unwrap_or(0) as isize;
        let next = (index + step).rem_euclid(keys.len() as isize) as usize;
        let target = keys[next].clone();
        self.switch_conversation(target.clone());
        if let Some(conv) = target {
            self.state.lock_safe().mark_conversation_read(&conv);
        }
    }

    /// Envoie un ReadReceipt pour chaque message reçu d'un autre pair dans la
    /// conversation donnée : pair (privé), `#nom` (salon) ou `None` (« Tous »).
    /// En salon/« Tous », l'accusé est diffusé à tous les membres en ligne
    /// pour que chacun voie le même détail « … » reçu/lu.
    pub(crate) fn send_read_receipts_for_conversation(&mut self, conv: Option<String>) {
        let s = self.state.lock_safe();
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
            for (recipient, addr) in s.receipt_recipients(m) {
                // Delta : ce destinataire a-t-il déjà reçu cet accusé ?
                if self
                    .read_receipts_sent
                    .get(&recipient)
                    .is_some_and(|sent| sent.contains(&hash))
                {
                    continue;
                }
                receipts.push(ReadReceiptRequest {
                    to_peer: recipient.clone(),
                    to_addr: addr,
                    receipt: ReadReceipt {
                        from: my_name.clone(),
                        to: recipient,
                        message_hash: hash,
                        timestamp: now.clone(),
                    },
                });
            }
        }
        drop(s);

        for req in receipts {
            let recipient = req.to_peer.clone();
            let hash = req.receipt.message_hash;
            // Marqué envoyé seulement si l'émission aboutit : l'inscrire avant
            // condamnait l'accusé à ne jamais partir dès que la file d'envoi
            // était pleine, puisqu'il était alors considéré comme déjà remis.
            if self.net.try_send(req) {
                self.read_receipts_sent
                    .entry(recipient)
                    .or_default()
                    .insert(hash);
            }
        }
    }
}

impl AbcomApp {
    /// Replie la fenêtre dans le tray : rendu stoppé, textures libérées
    /// (elles seront rechargées paresseusement à la réouverture). Sur macOS,
    /// l'application quitte aussi le Dock (politique Accessory) : elle ne
    /// vit plus que dans la barre de menus.
    pub(crate) fn hide_to_tray(&mut self, ctx: &egui::Context) {
        // Windows : on garde la fenêtre « visible » pour l'OS mais hors écran
        // et hors barre des tâches, sinon (SW_HIDE) egui cesse d'être appelé
        // et le menu du tray devient inerte. Ailleurs, masquage classique.
        #[cfg(windows)]
        tray::win::hide_offscreen();
        #[cfg(not(windows))]
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));

        set_dock_visible(false);
        self.window_hidden = true;
        self.window_focused = false;

        // Purge mémoire : textures GPU et caches d'images.
        self.media.textures.clear();
        self.media.texture_lru.clear();
        self.avatar_textures.clear();
        self.media.viewer_texture = None;
        self.media.viewer = None;
        for url in &self.media.known_gif_urls {
            ctx.forget_image(url);
        }
        self.forget_gif_previews(ctx);
        // Emojis : libérés aussi, re-décodés en arrière-plan au retour.
        self.emoji.textures.clear();
        self.chat_cache.invalidate();
        // Images du chargeur egui_extras (GIF, aperçus Klipy) : elles survivaient
        // au repli parce que seules les nôtres étaient libérées.
        ctx.forget_all_images();
        // Libérer ne suffit pas : sans ceci le RSS ne bouge pas.
        release_memory_to_os();
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
        #[cfg(windows)]
        tray::win::restore_onscreen();
        #[cfg(not(windows))]
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.chat_cache.invalidate();
        ctx.request_repaint();
    }

    /// Applique ce que les sélecteurs natifs ont rendu depuis la dernière
    /// frame. Le décodage d'un avatar a lieu ici, sur le thread de l'UI :
    /// l'image est petite et l'état qu'elle met à jour n'est pas partagé.
    fn apply_picker_outcomes(&mut self) {
        while let Ok(outcome) = self.picker_rx.try_recv() {
            match outcome {
                picker::PickerOutcome::Attachments(paths) => {
                    let label = if paths.len() == 1 && paths[0].is_dir() {
                        self.t(i18n::DOSSIER_AJOUTE)
                    } else {
                        self.t(i18n::FICHIERS_AJOUTES)
                    };
                    for path in paths {
                        if !self.composer.pending_attachments.contains(&path) {
                            self.composer.pending_attachments.push(path);
                        }
                    }
                    self.last_notification = Some(label.to_string());
                    self.notification_time = std::time::Instant::now();
                }
                picker::PickerOutcome::Export(path) => {
                    // Pas de notification ici : l'écriture est asynchrone et son
                    // verdict revient par `AppEvent::ConversationExported`.
                    self.state.lock_safe().export_selected_conversation(path);
                }
                picker::PickerOutcome::Avatar(path) => {
                    match avatar::load_normalized_avatar(&path) {
                        Ok(png) => {
                            let my_name = self.state.lock_safe().my_username.clone();
                            self.state.lock_safe().set_my_avatar(png);
                            self.avatar_textures.remove(&my_name);
                            self.broadcast_my_avatar();
                        }
                        Err(e) => {
                            tracing::warn!("avatar non chargé : {}", e);
                            self.last_notification =
                                Some(self.t(i18n::IMAGE_DE_PROFIL_INVALIDE).to_string());
                            self.notification_time = std::time::Instant::now();
                        }
                    }
                }
            }
        }
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
            self.t(i18n::NOUVEAU_MESSAGE).to_string()
        }
    }
}

impl eframe::App for AbcomApp {
    /// État et notifications : appelé aussi quand la fenêtre est repliée, sans aucune passe egui.
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // HWND natif capté une fois : permet de replier/restaurer la fenêtre
        // au niveau OS sous Windows (voir tray::win).
        #[cfg(windows)]
        tray::capture_window_handle(frame);
        #[cfg(not(windows))]
        let _ = &frame;

        // Icône résidente, créée paresseusement (macOS impose le thread
        // principal avec l'event loop démarrée — c'est le cas ici).
        if self.tray.is_none() && !self.tray_init_failed {
            self.tray = tray::Tray::new(self.t(i18n::OUVRIR_ABCOM), self.t(i18n::QUITTER));
            if self.tray.is_none() {
                self.tray_init_failed = true;
                tracing::warn!("icône résidente indisponible : la croix quittera l'application");
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

        let was_focused = self.window_focused;
        self.window_focused = !self.window_hidden && ctx.input(|i| i.focused);
        // Retour du focus : les messages arrivés pendant que la fenêtre était
        // en arrière-plan viennent d'être lus. Sans ce rattrapage, un message
        // reçu juste avant le clic ne recevait son accusé de lecture qu'après
        // avoir quitté la conversation et y être revenu — `switch_conversation`
        // était le seul déclencheur.
        if self.window_focused && !was_focused {
            let conv = self.state.lock_safe().selected_conversation.clone();
            self.send_read_receipts_for_conversation(conv.clone());
            if let Some(conv) = conv {
                self.state.lock_safe().mark_conversation_read(&conv);
            }
        }
        self.process_events();
        self.process_media_offers();
        self.periodic_tasks();

        // Badge non-lus sur l'icône résidente.
        let unread = self.has_unread;
        if let Some(t) = &mut self.tray {
            t.set_unread(unread);
        }
    }

    /// Rendu : jamais appelé fenêtre repliée, la logique vit dans [`Self::logic`].
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;

        // Minimisée : l'état est à jour, rien à peindre et aucun repaint programmé.
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if self.window_hidden || minimized {
            return;
        }

        ctx.set_theme(self.theme_preference);

        // Rafraîchit les caches dérivés si l'état a changé (génération) —
        // sinon la frame se rend sans reprendre le verrou ni rien re-dériver.
        {
            let s = self.state.lock_safe();
            self.sidebar_cache.refresh(&s);
            let rebuilt = self.chat_cache.refresh(
                &s,
                self.ui_language,
                &self.emoji.map,
                ctx.theme() == egui::Theme::Dark,
            );
            drop(s);
            if let Some(conv_changed) = rebuilt {
                if conv_changed {
                    self.chat_visible_count = CHAT_WINDOW_STEP;
                    self.chat_prepend_fix = None;
                }
                // Les GIFs sortis du fil (changement de conversation ou
                // expiration du ring-buffer) libèrent leurs frames décodées.
                for url in self
                    .media
                    .known_gif_urls
                    .difference(&self.chat_cache.gif_urls)
                {
                    ctx.forget_image(url);
                }
                self.media.known_gif_urls = self.chat_cache.gif_urls.clone();
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

        // Verdicts des sélecteurs natifs ouverts aux frames précédentes.
        self.apply_picker_outcomes();

        // Ouverture d'un sélecteur de fichiers ou de dossier. La fenêtre native
        // est présentée ici, sur le thread de l'UI ; l'attente, elle, part
        // ailleurs (cf. `picker`) — la bloquer ici tuait l'application.
        if self.pending_picker != 0 {
            let kind = self.pending_picker;
            self.pending_picker = 0;
            let (files_title, folder_title) = (
                self.t(i18n::AJOUTER_DES_FICHIERS),
                self.t(i18n::AJOUTER_UN_DOSSIER),
            );
            match kind {
                1 => {
                    let dialog = rfd::AsyncFileDialog::new()
                        .set_title(files_title)
                        .pick_files();
                    picker::spawn(self.picker_tx.clone(), ctx.clone(), async move {
                        let files = dialog.await?;
                        Some(picker::PickerOutcome::Attachments(
                            files.iter().map(|f| f.path().to_path_buf()).collect(),
                        ))
                    });
                }
                2 => {
                    let dialog = rfd::AsyncFileDialog::new()
                        .set_title(folder_title)
                        .pick_folder();
                    picker::spawn(self.picker_tx.clone(), ctx.clone(), async move {
                        let folder = dialog.await?;
                        Some(picker::PickerOutcome::Attachments(vec![folder
                            .path()
                            .to_path_buf()]))
                    });
                }
                _ => {}
            }
        }

        // Fichiers déposés dans la fenêtre : egui les expose, il ne restait
        // qu'à les brancher sur le pipeline de pièces jointes existant.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            let added = self.t(i18n::FICHIERS_AJOUTES);
            for path in dropped {
                if !self.composer.pending_attachments.contains(&path) {
                    self.composer.pending_attachments.push(path);
                }
            }
            self.last_notification = Some(added.to_string());
            self.notification_time = std::time::Instant::now();
        }

        // Export de conversation : même sélecteur asynchrone que les autres.
        if self.pending_export {
            self.pending_export = false;
            let title = self.t(i18n::EXPORTER_LA_CONVERSATION);
            let name = {
                let state = self.state.lock_safe();
                match &state.selected_conversation {
                    Some(conv) => conv.trim_start_matches('#').to_string(),
                    None => "tous".to_string(),
                }
            };
            let dialog = rfd::AsyncFileDialog::new()
                .set_title(title)
                .set_file_name(format!("abcom-{name}.txt"))
                .save_file();
            picker::spawn(self.picker_tx.clone(), ctx.clone(), async move {
                Some(picker::PickerOutcome::Export(
                    dialog.await?.path().to_path_buf(),
                ))
            });
        }

        // Sélection de l'image de profil.
        if self.pending_avatar_pick {
            self.pending_avatar_pick = false;
            let dialog = rfd::AsyncFileDialog::new()
                .set_title(self.t(i18n::CHOISIR_UNE_IMAGE_DE_PROFIL))
                .add_filter("Images", &["png", "jpg", "jpeg", "svg"])
                .pick_file();
            picker::spawn(self.picker_tx.clone(), ctx.clone(), async move {
                Some(picker::PickerOutcome::Avatar(
                    dialog.await?.path().to_path_buf(),
                ))
            });
        }

        // Avant les panneaux : une combinaison consommée ici ne sera pas
        // réinterprétée par un widget dans la même frame.
        self.handle_shortcuts(ctx);
        self.submit_search();

        // Ordre imposé par egui : panneaux latéraux, puis bas, puis central.
        self.show_sidebar_panel(root);
        let (emoji_btn_clicked, gif_btn_clicked) = self.show_input_bar(root);
        self.show_notification(ctx);
        self.show_emoji_picker_window(ctx, emoji_btn_clicked);
        self.show_gif_picker_window(ctx, gif_btn_clicked);
        self.render_group_modal(ctx);
        self.render_group_manage_modal(ctx);
        self.show_central_panel(root);
        self.show_reaction_emoji_picker(ctx);
        self.render_settings(ctx);
        self.show_media_viewer(ctx);
        self.show_search(ctx);

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
    fn on_exit(&mut self) {
        if let Err(error) = self.state.lock_safe().flush_storage() {
            // Dernier instant utile : la fenêtre se ferme, il n'y a plus d'UI
            // pour prévenir. Le journal garde la trace de ce qui n'a pas été
            // écrit, au lieu de laisser croire à une sauvegarde réussie.
            tracing::error!("historique non sauvegardé : {error}");
        }
    }
}

/// Nom de la famille de police en gras enregistrée dans egui (Inter Bold).
/// egui ne synthétise pas le gras : on charge une vraie police pour les noms.
pub(crate) const BOLD_FAMILY: &str = "bold";

/// Définitions de polices : on conserve les polices par défaut et on ajoute
/// Inter Bold (OFL) sous la famille [`BOLD_FAMILY`] pour les noms d'auteur.
///
/// Inter sert aussi de **repli** aux familles standard. Les polices par défaut
/// d'egui ignorent des caractères courants dès qu'on colle du texte venu
/// d'ailleurs — coche `✓`, flèche `→`, retour `⏎` — qui s'affichaient alors en
/// carré vide. Placée en dernier, Inter n'est consultée que pour ce que les
/// autres ne savent pas dessiner : le texte ordinaire garde sa graisse.
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
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("inter-bold".to_owned());
    }
    fonts
}

fn app_icon_data() -> Option<egui::IconData> {
    let data = include_bytes!("../../assets/app_icon.png");
    tracing::debug!("chargement icône PNG ({} bytes)", data.len());
    match image::load_from_memory(data) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            tracing::debug!("icône chargée : {}x{}", w, h);
            Some(egui::IconData {
                rgba: rgba.to_vec(),
                width: w,
                height: h,
            })
        }
        Err(err) => {
            tracing::warn!("erreur icône PNG : {}", err);
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

/// Point d'entrée de l'interface graphique.
pub fn run(
    state: Arc<Mutex<AppState>>,
    ui_ctx: crate::platform::notify::UiContext,
    identity_fingerprint: String,
    psk_active: bool,
    channels: UiRuntimeChannels,
) -> anyhow::Result<()> {
    // Handlers tray/menu globaux : chaque événement réveille l'UI via le
    // contexte partagé (fonctionne même fenêtre cachée, sans rendu).
    tray::install_event_handlers(ui_ctx.clone());

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Abcom")
        .with_inner_size([860.0, 600.0])
        // En dessous, la barre latérale seule occupe toute la fenêtre.
        .with_min_inner_size([560.0, 360.0]);

    // Windows : le glisser-déposer OLE de winit entre en conflit avec les
    // boîtes de dialogue COM de rfd. À revoir si rfd passe en asynchrone.
    #[cfg(windows)]
    {
        viewport = viewport.with_drag_and_drop(false);
    }

    if let Some(icon) = app_icon_data() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        // Le défaut d'egui-wgpu est `HighPerformance`, ce qui réveille le GPU
        // dédié d'un portable pour peindre du texte et des rectangles.
        wgpu_options: low_power_wgpu(),
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
            // Libère la copie CPU des images une fois téléversées en GPU.
            cc.egui_ctx
                .options_mut(|options| options.reduce_texture_memory = true);
            Ok(Box::new(AbcomApp::new(
                state,
                identity_fingerprint,
                psk_active,
                channels,
            )))
        }),
    )
    .map_err(|e| {
        tracing::error!("erreur GUI : {}", e);
        tracing::error!("sur WSL sans GPU, utilisez make run-windows.");
        anyhow::anyhow!("Échec GUI : {}", e)
    })?;

    Ok(())
}

/// Raccourcis globaux de l'application, déclarés en un seul endroit.
///
/// `consume_shortcut` réserve la combinaison pour nous et empêche qu'un autre
/// widget la traite dans la même frame — ce que notre filtrage manuel de
/// touches, cantonné au composeur, ne savait pas faire.
pub mod shortcuts {
    use eframe::egui::{Key, KeyboardShortcut, Modifiers};

    pub const SETTINGS: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma);
    pub const NEXT_CONVERSATION: KeyboardShortcut =
        KeyboardShortcut::new(Modifiers::CTRL, Key::Tab);
    pub const PREV_CONVERSATION: KeyboardShortcut =
        KeyboardShortcut::new(Modifiers::CTRL.plus(Modifiers::SHIFT), Key::Tab);
    pub const SEARCH: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);
    pub const CLOSE_OVERLAY: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Escape);
}

/// Configuration wgpu privilégiant le GPU intégré : le défaut d'egui-wgpu est
/// `HighPerformance`, ce qui réveille la carte dédiée pour une interface 2D.
fn low_power_wgpu() -> eframe::egui_wgpu::WgpuConfiguration {
    let mut options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_setup {
        setup.power_preference = eframe::wgpu::PowerPreference::LowPower;
    }
    options
}

/// Rend à l'OS les pages libérées mais retenues par l'allocateur (sans effet hors mimalloc).
pub(crate) fn release_memory_to_os() {
    // SAFETY : mi_collect est thread-safe et sans précondition ; force = true rend les pages.
    unsafe {
        libmimalloc_sys::mi_collect(true);
    }
}

/// macOS : montre/retire l'icône du Dock. Repliée dans la barre de menus,
/// l'application passe en politique `Accessory` (plus de Dock ni de Cmd-Tab) ;
/// à la réouverture elle redevient `Regular` et revient au premier plan.
/// Doit être appelé sur le thread principal (c'est le cas dans `update`).
#[cfg(target_os = "macos")]
fn set_dock_visible(visible: bool) {
    // objc2 0.6 : `alloc()` vient du trait `AnyThread` (ex-`ClassType`).
    use objc2::AnyThread;
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

#[cfg(test)]
#[path = "../tests/test_ui_app.rs"]
mod app_tests;

#[cfg(test)]
#[path = "../tests/test_ui_fonts.rs"]
mod font_tests;
