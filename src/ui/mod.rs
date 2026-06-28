use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{
    AppEvent, AvatarRequest, MediaProgress, MediaSendJob, MediaStreamOffer, MessageAckRequest,
    ReadReceipt, ReadReceiptRequest, SendGroupRequest, SendRequest, TypingRequest,
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
mod settings;
mod sidebar;
mod sound;

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
    pub(crate) event_rx: mpsc::Receiver<AppEvent>,
    pub(crate) send_tx: mpsc::Sender<SendRequest>,
    pub(crate) send_group_tx: mpsc::Sender<SendGroupRequest>,
    pub(crate) send_typing_tx: mpsc::Sender<TypingRequest>,
    pub(crate) send_read_receipt_tx: mpsc::Sender<ReadReceiptRequest>,
    pub(crate) send_ack_tx: mpsc::Sender<MessageAckRequest>,
    pub(crate) send_avatar_tx: mpsc::Sender<AvatarRequest>,
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
    pub(crate) last_cleanup_time: std::time::Instant,
    pub(crate) show_group_modal: bool,
    pub(crate) group_name_input: String,
    pub(crate) group_members_selected: std::collections::HashSet<String>,
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
}

impl AbcomApp {
    // Câblage des canaux mpsc indépendants vers les tâches réseau/transfert.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        state: Arc<Mutex<AppState>>,
        event_rx: mpsc::Receiver<AppEvent>,
        send_tx: mpsc::Sender<SendRequest>,
        send_group_tx: mpsc::Sender<SendGroupRequest>,
        send_typing_tx: mpsc::Sender<TypingRequest>,
        send_read_receipt_tx: mpsc::Sender<ReadReceiptRequest>,
        send_ack_tx: mpsc::Sender<MessageAckRequest>,
        send_avatar_tx: mpsc::Sender<AvatarRequest>,
        send_media_tx: mpsc::Sender<MediaSendJob>,
        media_offer_rx: mpsc::Receiver<MediaStreamOffer>,
    ) -> Self {
        Self {
            state,
            event_rx,
            send_tx,
            send_group_tx,
            send_typing_tx,
            send_read_receipt_tx,
            send_ack_tx,
            send_avatar_tx,
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
            last_cleanup_time: std::time::Instant::now(),
            show_group_modal: false,
            group_name_input: String::new(),
            group_members_selected: std::collections::HashSet::new(),
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

        // Accusés de lecture (différés) pour tous les messages reçus dans la
        // conversation ouverte — privée, de groupe ou diffusée (« Tous »).
        self.send_read_receipts_for_conversation(&new_conversation);
    }

    /// Envoie un ReadReceipt pour chaque message reçu d'un autre pair dans la
    /// conversation donnée, vers tous les destinataires concernés (l'expéditeur
    /// en privé, tout le groupe en `#…`, tous les pairs en « Tous »).
    pub(crate) fn send_read_receipts_for_conversation(&mut self, conv: &Option<String>) {
        let s = self.state.lock().unwrap();
        let my_name = s.my_username.clone();
        let now = chrono::Local::now().format("%H:%M").to_string();

        // Un message appartient-il à cette conversation et provient-il d'autrui ?
        let belongs = |m: &crate::message::ChatMessage| -> bool {
            match conv {
                None => m.to_user.is_none() && m.from != my_name,
                Some(g) if g.starts_with('#') => {
                    m.to_user.as_deref() == Some(g.as_str()) && m.from != my_name
                }
                Some(peer) => m.from == *peer && m.to_user.as_deref() == Some(my_name.as_str()),
            }
        };

        let mut reqs: Vec<ReadReceiptRequest> = Vec::new();
        for m in s.messages.iter().filter(|m| belongs(m)) {
            let hash = crate::app::AppState::message_hash(m);
            for addr in s.receipt_recipients(m) {
                reqs.push(ReadReceiptRequest {
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

        for req in reqs {
            let _ = self.send_read_receipt_tx.try_send(req);
        }
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme_preference(ctx);
        let was_focused = self.window_focused;
        self.window_focused = ctx.input(|i| i.focused);

        self.lazy_load_emoji(ctx);
        self.process_events();
        self.process_media_offers();
        self.periodic_tasks();

        // Reprise du focus : l'utilisateur revient sur la fenêtre et « lit » donc
        // la conversation ouverte. On (re)renvoie les accusés de lecture pour
        // tous ses messages reçus — indispensable quand le message est arrivé
        // fenêtre en arrière-plan (cas courant : deux instances sur un même poste,
        // une seule peut avoir le focus système à la fois).
        if !was_focused && self.window_focused {
            let conv = self.state.lock().unwrap().selected_conversation.clone();
            self.send_read_receipts_for_conversation(&conv);
        }

        // Flash barre des tâches si message non lu — réinitialisé une seule fois
        // quand la fenêtre reprend le focus (pas d'envoi répété en boucle).
        if self.has_unread && ctx.input(|i| i.focused) {
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
        self.show_central_panel(ctx);
        self.render_settings(ctx);
        self.show_media_viewer(ctx);

        ctx.request_repaint_after(Duration::from_millis(500));
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
    event_rx: mpsc::Receiver<AppEvent>,
    send_tx: mpsc::Sender<SendRequest>,
    send_group_tx: mpsc::Sender<SendGroupRequest>,
    send_typing_tx: mpsc::Sender<TypingRequest>,
    send_read_receipt_tx: mpsc::Sender<ReadReceiptRequest>,
    send_ack_tx: mpsc::Sender<MessageAckRequest>,
    send_avatar_tx: mpsc::Sender<AvatarRequest>,
    send_media_tx: mpsc::Sender<MediaSendJob>,
    media_offer_rx: mpsc::Receiver<MediaStreamOffer>,
) -> anyhow::Result<()> {
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
        Box::new(|cc| {
            cc.egui_ctx.set_fonts(build_fonts());
            // Loaders d'images egui_extras : HTTP (récupération depuis le CDN
            // Klipy) + décodage GIF/WebP animés pour les vignettes et le fil.
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(AbcomApp::new(
                state,
                event_rx,
                send_tx,
                send_group_tx,
                send_typing_tx,
                send_read_receipt_tx,
                send_ack_tx,
                send_avatar_tx,
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
