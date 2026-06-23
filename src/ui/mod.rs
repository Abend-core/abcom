use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{
    AppEvent, MessageAckRequest, ReadReceiptRequest, SendGroupRequest, SendRequest, TypingRequest,
};
use crate::transfer::{TransferDecision, TransferOffer, TransferProgress, TransferRequest};

mod chat_panel;
mod settings;
pub mod composer;
mod emoji_picker;
mod events;
mod group_modal;
mod input_bar;
mod markdown;
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
    General,
    Credits,
    License,
}

/// Proposition de réception en attente d'une décision de l'utilisateur.
pub(crate) struct PendingOffer {
    pub(crate) transfer_id: String,
    pub(crate) from: String,
    pub(crate) label: String,
    pub(crate) total_bytes: u64,
    pub(crate) item_count: usize,
    pub(crate) decision_tx: tokio::sync::oneshot::Sender<TransferDecision>,
    pub(crate) received_at: std::time::Instant,
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
    pub(crate) send_transfer_tx: mpsc::Sender<TransferRequest>,
    pub(crate) input: String,
    pub(crate) input_cursor_char: usize,
    pub(crate) input_selection_anchor: Option<usize>,
    pub(crate) input_has_focus: bool,
    pub(crate) input_scroll_lines: f32,
    pub(crate) show_attachment_menu: bool,
    pub(crate) show_emoji_picker: bool,
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
    pub(crate) transfer_progress: std::collections::HashMap<String, TransferProgress>,
    /// Transferts entrants en attente d'acceptation/refus.
    pub(crate) offer_rx: mpsc::Receiver<TransferOffer>,
    pub(crate) pending_offers: Vec<PendingOffer>,
    /// Transferts masqués par l'utilisateur (croix de fermeture).
    pub(crate) dismissed_transfers: std::collections::HashSet<String>,
    /// transfer_id d'une offre acceptée en attente de choix du dossier (différé).
    pub(crate) pending_accept: Option<String>,
    pub(crate) ui_language: UiLanguage,
    pub(crate) theme_preference: ThemePreference,
    pub(crate) system_dark_mode: Option<bool>,
    pub(crate) show_settings: bool,
    pub(crate) settings_tab: SettingsTab,
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
        send_transfer_tx: mpsc::Sender<TransferRequest>,
        offer_rx: mpsc::Receiver<TransferOffer>,
    ) -> Self {
        Self {
            state,
            event_rx,
            send_tx,
            send_group_tx,
            send_typing_tx,
            send_read_receipt_tx,
            send_ack_tx,
            send_transfer_tx,
            offer_rx,
            pending_offers: Vec::new(),
            dismissed_transfers: std::collections::HashSet::new(),
            pending_accept: None,
            input: String::new(),
            input_cursor_char: 0,
            input_selection_anchor: None,
            input_has_focus: false,
            input_scroll_lines: 0.0,
            show_attachment_menu: false,
            show_emoji_picker: false,
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
            transfer_progress: std::collections::HashMap::new(),
            ui_language: UiLanguage::French,
            theme_preference: ThemePreference::System,
            system_dark_mode: None,
            show_settings: false,
            settings_tab: SettingsTab::General,
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
        self.load_draft(new_conversation);
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme_preference(ctx);
        self.window_focused = ctx.input(|i| i.focused);

        self.lazy_load_emoji(ctx);
        self.process_events();
        self.process_transfer_offers();
        self.periodic_tasks();

        // Flash barre des tâches si message non lu
        if self.has_unread {
            if !ctx.input(|i| i.focused) {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Informational,
                ));
            } else {
                self.has_unread = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Reset,
                ));
            }
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
                    if let Some(paths) = rfd::FileDialog::new().set_title(files_title).pick_files() {
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
                    if let Some(path) = rfd::FileDialog::new().set_title(folder_title).pick_folder() {
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

        // Choix du dossier de réception après acceptation d'un fichier (différé
        // pour ne pas entrer en conflit avec la run-loop AppKit sur macOS).
        if let Some(transfer_id) = self.pending_accept.take() {
            let title = self.tr("Choisir le dossier de réception", "Choose destination folder");
            if let Some(dir) = rfd::FileDialog::new().set_title(title).pick_folder() {
                if let Some(pos) = self
                    .pending_offers
                    .iter()
                    .position(|o| o.transfer_id == transfer_id)
                {
                    let offer = self.pending_offers.remove(pos);
                    let _ = offer.decision_tx.send(TransferDecision {
                        accept: true,
                        dest_dir: Some(dir),
                    });
                }
            }
            // Si l'utilisateur annule le sélecteur, l'offre reste affichée.
        }

        self.show_sidebar_panel(ctx);
        let emoji_btn_clicked = self.show_input_bar(ctx);
        self.show_notification(ctx);
        self.show_emoji_picker_window(ctx, emoji_btn_clicked);
        self.render_group_modal(ctx);
        self.show_central_panel(ctx);
        self.render_settings(ctx);

        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn app_icon_data() -> Option<egui::IconData> {
    let data = include_bytes!("../../Sans titre.png");
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
    send_transfer_tx: mpsc::Sender<TransferRequest>,
    offer_rx: mpsc::Receiver<TransferOffer>,
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
        Box::new(|_cc| {
            Ok(Box::new(AbcomApp::new(
                state,
                event_rx,
                send_tx,
                send_group_tx,
                send_typing_tx,
                send_read_receipt_tx,
                send_ack_tx,
                send_transfer_tx,
                offer_rx,
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
