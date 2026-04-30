use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{AppEvent, ReadReceiptRequest, MessageAckRequest, SendGroupRequest, SendRequest, TypingRequest};

pub mod composer;
mod chat_panel;
mod emoji_picker;
mod events;
mod group_modal;
mod input_bar;
mod networks_view;
mod sidebar;
mod sound;

/// Vue active dans la zone centrale
#[derive(PartialEq, Clone)]
pub(crate) enum AppView {
    Chat,
    Networks,
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
    pub(crate) input: String,
    pub(crate) input_cursor_char: usize,
    pub(crate) input_has_focus: bool,
    pub(crate) input_scroll_lines: f32,
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
    pub(crate) last_network_check: std::time::Instant,
    pub(crate) show_group_modal: bool,
    pub(crate) group_name_input: String,
    pub(crate) group_members_selected: std::collections::HashSet<String>,
    pub(crate) last_typing_broadcast: std::time::Instant,
    pub(crate) last_retry_time: std::time::Instant,
    pub(crate) muted_conversations: std::collections::HashSet<Option<String>>,
    pub(crate) selected_network_filter: Option<String>,
    pub(crate) active_view: AppView,
    pub(crate) networks_view_selected: Option<String>,
    pub(crate) editing_network_alias: Option<(String, String)>,
    pub(crate) editing_peer_alias: Option<(String, String)>,
    pub(crate) selected_network_view: Option<String>,
    pub(crate) network_alias_edits: std::collections::HashMap<String, String>,
    pub(crate) peer_alias_edits: std::collections::HashMap<String, String>,
}

impl AbcomApp {
    pub(crate) fn new(
        state: Arc<Mutex<AppState>>,
        event_rx: mpsc::Receiver<AppEvent>,
        send_tx: mpsc::Sender<SendRequest>,
        send_group_tx: mpsc::Sender<SendGroupRequest>,
        send_typing_tx: mpsc::Sender<TypingRequest>,
        send_read_receipt_tx: mpsc::Sender<ReadReceiptRequest>,
        send_ack_tx: mpsc::Sender<MessageAckRequest>,
    ) -> Self {
        Self {
            state,
            event_rx,
            send_tx,
            send_group_tx,
            send_typing_tx,
            send_read_receipt_tx,
            send_ack_tx,
            input: String::new(),
            input_cursor_char: 0,
            input_has_focus: false,
            input_scroll_lines: 0.0,
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
            last_network_check: std::time::Instant::now() - Duration::from_secs(15),
            show_group_modal: false,
            group_name_input: String::new(),
            group_members_selected: std::collections::HashSet::new(),
            last_typing_broadcast: std::time::Instant::now(),
            last_retry_time: std::time::Instant::now(),
            muted_conversations: std::collections::HashSet::new(),
            selected_network_filter: None,
            active_view: AppView::Chat,
            networks_view_selected: None,
            editing_network_alias: None,
            editing_peer_alias: None,
            selected_network_view: None,
            network_alias_edits: std::collections::HashMap::new(),
            peer_alias_edits: std::collections::HashMap::new(),
        }
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.window_focused = ctx.input(|i| i.focused);

        self.lazy_load_emoji(ctx);
        self.process_events();
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

        self.show_sidebar_panel(ctx);
        self.show_typing_panel(ctx);
        let emoji_btn_clicked = self.show_input_bar(ctx);
        self.show_notification(ctx);
        self.show_emoji_picker_window(ctx, emoji_btn_clicked);
        self.render_group_modal(ctx);
        self.show_central_panel(ctx);

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
            Some(egui::IconData { rgba: rgba.to_vec(), width: w, height: h })
        }
        Err(err) => {
            eprintln!("[ui] Erreur icône PNG : {}", err);
            let mut rgba = vec![0u8; 32 * 32 * 4];
            for i in 0..(32 * 32) {
                rgba[i * 4] = 200; rgba[i * 4 + 1] = 50; rgba[i * 4 + 2] = 50; rgba[i * 4 + 3] = 255;
            }
            Some(egui::IconData { rgba, width: 32, height: 32 })
        }
    }
}

/// Point d'entrée de l'interface graphique
pub fn run(
    state: Arc<Mutex<AppState>>,
    event_rx: mpsc::Receiver<AppEvent>,
    send_tx: mpsc::Sender<SendRequest>,
    send_group_tx: mpsc::Sender<SendGroupRequest>,
    send_typing_tx: mpsc::Sender<TypingRequest>,
    send_read_receipt_tx: mpsc::Sender<ReadReceiptRequest>,
    send_ack_tx: mpsc::Sender<MessageAckRequest>,
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
                state, event_rx, send_tx, send_group_tx,
                send_typing_tx, send_read_receipt_tx, send_ack_tx,
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
