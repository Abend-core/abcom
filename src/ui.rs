use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::time::Duration;

use eframe::egui;
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::message::{AppEvent, ChatMessage, SendRequest, SendGroupRequest, SendTypingRequest};
use crate::transfer::model::{TransferDirection, TransferRecord, TransferStatus};
use crate::transfer::service::TransferCommand;

fn app_icon_data() -> Option<egui::IconData> {
    let data = include_bytes!("../assets/app_icon.jpg");
    eprintln!("[ui] Tentative de chargement de l'icône JPG ({} bytes)", data.len());
    match image::load_from_memory(data) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            eprintln!("[ui] Icône chargée avec succès : {}x{}", w, h);
            Some(egui::IconData {
                rgba: rgba.to_vec(),
                width: w,
                height: h,
            })
        }
        Err(err) => {
            eprintln!("[ui] Erreur de chargement icône JPG : {}", err);
            // Fallback: créer une simple icône 32x32 rouge
            let mut rgba = vec![0u8; 32 * 32 * 4];
            for i in 0..(32 * 32) {
                rgba[i * 4] = 200;     // R
                rgba[i * 4 + 1] = 50;  // G
                rgba[i * 4 + 2] = 50;  // B
                rgba[i * 4 + 3] = 255; // A
            }
            eprintln!("[ui] Utilisation de l'icône par défaut (carrée rouge)");
            Some(egui::IconData {
                rgba,
                width: 32,
                height: 32,
            })
        }
    }
}

pub fn run(
    state: Arc<Mutex<AppState>>,
    event_rx: mpsc::Receiver<AppEvent>,
    send_tx: mpsc::Sender<SendRequest>,
    send_group_tx: mpsc::Sender<SendGroupRequest>,
    typing_tx: mpsc::Sender<SendTypingRequest>,
    transfer_cmd_tx: mpsc::Sender<TransferCommand>,
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
                state.clone(),
                event_rx,
                send_tx.clone(),
                send_group_tx.clone(),
                typing_tx.clone(),
                transfer_cmd_tx.clone(),
            )))
        }),
    )
    .map_err(|e| {
        eprintln!("Erreur lors du lancement de l'interface graphique : {}", e);
        eprintln!("Cela peut être dû à un environnement graphique non supporté (par exemple WSL sans pilote OpenGL approprié).");
        eprintln!("Pour déployer sur Windows, utilisez le script d'installation : scripts/install-windows.ps1");
        anyhow::anyhow!("Échec de l'initialisation de l'interface graphique : {}", e)
    })?;

    Ok(())
}

/// Vue active dans la zone centrale
#[derive(PartialEq, Clone)]
enum AppView {
    Chat,
    Networks,
}

struct AbcomApp {
    state: Arc<Mutex<AppState>>,
    event_rx: mpsc::Receiver<AppEvent>,
    send_tx: mpsc::Sender<SendRequest>,
    send_group_tx: mpsc::Sender<SendGroupRequest>,
    transfer_cmd_tx: mpsc::Sender<TransferCommand>,
    input: String,
    input_cursor_char: usize,
    input_has_focus: bool,
    input_scroll_lines: f32,
    show_emoji_picker: bool,
    show_participants: bool,
    enable_sound_notifications: bool,
    last_notification: Option<String>,
    notification_time: std::time::Instant,
    has_unread: bool,
    window_focused: bool,
    emoji_textures: Vec<(String, egui::TextureHandle)>,
    emoji_textures_loaded: bool,
    emoji_category: usize,
    emoji_map: std::collections::HashMap<String, usize>,
    emoji_alias_to_char: std::collections::HashMap<String, String>,
    emoji_aliases: Vec<String>,
    shortcode_selected: usize,
    last_cleanup_time: std::time::Instant,
    last_network_check: std::time::Instant,
    // Gestion des groupes
    show_group_modal: bool,
    group_name_input: String,
    group_members_selected: std::collections::HashSet<String>,
    // Indicateur de frappe
    typing_tx: mpsc::Sender<SendTypingRequest>,
    last_typing_sent: std::time::Instant,
    // Sons désactivés par salon (clé = nom du salon, None = Global)
    muted_conversations: std::collections::HashSet<Option<String>>,
    // Gestion des réseaux
    /// Subnet sélectionné pour filtrer le panel gauche (None = réseau actuel)
    selected_network_filter: Option<String>,
    /// Vue active dans la zone centrale ("chat" | "networks")
    active_view: AppView,
    /// Réseau sélectionné dans la vue réseaux (pour voir ses pairs)
    networks_view_selected: Option<String>,
    /// Édition alias réseau
    editing_network_alias: Option<(String, String)>, // (subnet, buffer)
    /// Édition alias pair
    editing_peer_alias: Option<(String, String)>, // (username, buffer)
    share_selection: Vec<PathBuf>,
    show_share_modal: bool,
    share_error: Option<String>,
}

fn load_emoji_textures(ctx: &egui::Context) -> Vec<(String, egui::TextureHandle)> {
    crate::emoji_registry::EMOJI_DATA
        .iter()
        .filter_map(|(ch, bytes)| {
            image::load_from_memory(bytes).ok().map(|img| {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    format!("emoji_{ch}"),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                (ch.to_string(), texture)
            })
        })
        .collect()
}

fn github_key_to_emoji(key: &str) -> Option<String> {
    let mut out = String::new();
    for part in key.split('-') {
        let cp = u32::from_str_radix(part, 16).ok()?;
        out.push(char::from_u32(cp)?);
    }
    Some(out)
}

fn build_emoji_shortcode_index(
    available: &[String],
) -> (std::collections::HashMap<String, String>, Vec<String>) {
    let available_set: std::collections::HashSet<String> = available.iter().cloned().collect();
    let Ok(raw): Result<std::collections::HashMap<String, serde_json::Value>, _> =
        serde_json::from_str(include_str!("../assets/github.raw.json"))
    else {
        return (std::collections::HashMap::new(), Vec::new());
    };

    let mut alias_to_char = std::collections::HashMap::new();
    for (key, value) in raw {
        let Some(emoji) = github_key_to_emoji(&key) else {
            continue;
        };
        if !available_set.contains(&emoji) {
            continue;
        }

        match value {
            serde_json::Value::String(alias) => {
                let a = alias.to_lowercase();
                alias_to_char.entry(a).or_insert_with(|| emoji.clone());
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    if let Some(alias) = v.as_str() {
                        let a = alias.to_lowercase();
                        alias_to_char.entry(a).or_insert_with(|| emoji.clone());
                    }
                }
            }
            _ => {}
        }
    }

    let mut aliases: Vec<String> = alias_to_char.keys().cloned().collect();
    aliases.sort();
    (alias_to_char, aliases)
}

fn emoji_shortcode_trigger(input: &str, cursor_char: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    if cursor_char > chars.len() {
        return None;
    }

    let mut start = cursor_char;
    while start > 0 {
        let ch = chars[start - 1];
        if ch.is_whitespace() {
            break;
        }
        start -= 1;
    }

    if start >= chars.len() || chars[start] != ':' {
        return None;
    }
    let query: String = chars[start + 1..cursor_char].iter().collect();
    if !query
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '+')
    {
        return None;
    }
    Some((start, query.to_lowercase()))
}

fn shortcode_suggestions(
    input: &str,
    cursor_char: usize,
    alias_to_char: &std::collections::HashMap<String, String>,
    aliases: &[String],
    limit: usize,
) -> Vec<(String, String)> {
    let Some((_start, query)) = emoji_shortcode_trigger(input, cursor_char) else {
        return Vec::new();
    };

    aliases
        .iter()
        .filter(|a| a.starts_with(&query))
        .take(limit)
        .filter_map(|a| alias_to_char.get(a).map(|ch| (a.clone(), ch.clone())))
        .collect()
}

fn replace_char_range(
    input: &mut String,
    cursor_char: &mut usize,
    start_char: usize,
    end_char: usize,
    replacement: &str,
) {
    let start = char_to_byte_idx(input, start_char);
    let end = char_to_byte_idx(input, end_char);
    input.replace_range(start..end, replacement);
    *cursor_char = start_char + replacement.chars().count();
}


/// Rend un texte contenant des emojis en les affichant comme images PNG colorées.
fn render_inline(
    ui: &mut egui::Ui,
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    textures: &[(String, egui::TextureHandle)],
    emoji_size: f32,
) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut acc = String::new();
    let size = egui::vec2(emoji_size, emoji_size);

    while i < chars.len() {
        let mut matched = false;
        // Essayer séquences de 2 chars (drapeaux, ZWJ) puis 1 char
        for len in [2usize, 1] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if let Some(&idx) = emoji_map.get(&s) {
                    if !acc.is_empty() {
                        ui.label(&acc);
                        acc.clear();
                    }
                    if let Some((_, tex)) = textures.get(idx) {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(size));
                    }
                    i += len;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            let ch = chars[i];
            // Ignorer les variation selectors (FE0F) qui ne s'affichent pas
            if ch != '\u{fe0f}' && ch != '\u{200d}' {
                acc.push(ch);
            }
            i += 1;
        }
    }
    if !acc.is_empty() {
        ui.label(&acc);
    }
}

fn char_to_byte_idx(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or_else(|| text.len())
}

fn insert_text_at_cursor(input: &mut String, cursor_char: &mut usize, text: &str) {
    let byte_idx = char_to_byte_idx(input, *cursor_char);
    input.insert_str(byte_idx, text);
    *cursor_char += text.chars().count();
}

fn remove_prev_char(input: &mut String, cursor_char: &mut usize) -> bool {
    if *cursor_char == 0 {
        return false;
    }
    let start = char_to_byte_idx(input, *cursor_char - 1);
    let end = char_to_byte_idx(input, *cursor_char);
    input.replace_range(start..end, "");
    *cursor_char -= 1;
    true
}

fn remove_next_char(input: &mut String, cursor_char: &mut usize) -> bool {
    let total = input.chars().count();
    if *cursor_char >= total {
        return false;
    }
    let start = char_to_byte_idx(input, *cursor_char);
    let end = char_to_byte_idx(input, *cursor_char + 1);
    input.replace_range(start..end, "");
    true
}

fn insert_emoji_at_cursor(input: &mut String, cursor_char: &mut usize, emoji: &str) {
    insert_text_at_cursor(input, cursor_char, emoji);
}

fn format_transfer_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let value = bytes as f64;
    if value >= GB {
        format!("{:.1} Go", value / GB)
    } else if value >= MB {
        format!("{:.1} Mo", value / MB)
    } else if value >= KB {
        format!("{:.1} Ko", value / KB)
    } else {
        format!("{} o", bytes)
    }
}

fn transfer_status_label(status: &TransferStatus) -> (&'static str, egui::Color32) {
    match status {
        TransferStatus::Preparing => ("Préparation", egui::Color32::from_rgb(110, 110, 110)),
        TransferStatus::WaitingForPeer => ("En attente", egui::Color32::from_rgb(180, 140, 30)),
        TransferStatus::Transferring => ("Transfert", egui::Color32::from_rgb(60, 120, 210)),
        TransferStatus::Completed => ("Terminé", egui::Color32::from_rgb(40, 150, 85)),
        TransferStatus::Failed => ("Échec", egui::Color32::from_rgb(210, 70, 70)),
    }
}

fn push_unique_paths(target: &mut Vec<PathBuf>, paths: impl IntoIterator<Item = PathBuf>) {
    for path in paths {
        if !target.iter().any(|existing| existing == &path) {
            target.push(path);
        }
    }
}

fn render_transfer_card(ui: &mut egui::Ui, transfer: &TransferRecord) {
    let (status_text, status_color) = transfer_status_label(&transfer.status);
    let direction_icon = match transfer.direction {
        TransferDirection::Outgoing => "⬆",
        TransferDirection::Incoming => "⬇",
    };
    let progress_text = format!(
        "{} / {}",
        format_transfer_bytes(transfer.transferred_bytes),
        format_transfer_bytes(transfer.total_bytes),
    );

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(direction_icon);
            ui.label(egui::RichText::new(&transfer.label).strong());
            ui.label(egui::RichText::new(format!("avec {}", transfer.peer_username)).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.colored_label(status_color, status_text);
            });
        });
        ui.weak(format!("Salon : {}", transfer.conversation.display_label()));
        ui.add(
            egui::ProgressBar::new(transfer.progress_ratio())
                .desired_width(f32::INFINITY)
                .text(progress_text),
        );
        if let Some(current_path) = &transfer.current_path {
            ui.label(egui::RichText::new(current_path).small());
        }
        if let Some(destination_root) = &transfer.destination_root {
            if matches!(transfer.direction, TransferDirection::Incoming) {
                ui.weak(format!("Destination : {}", destination_root.display()));
            }
        }
        if let Some(error) = &transfer.error {
            ui.colored_label(egui::Color32::from_rgb(210, 70, 70), error);
        }
    });
}

fn measure_text_width(ui: &egui::Ui, text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font_id, ui.visuals().text_color())
        .size()
        .x
}

fn composer_caret_positions(
    ui: &egui::Ui,
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_size: f32,
) -> Vec<egui::Pos2> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let line_height = 22.0;
    let mut x = 0.0;
    let mut y = 0.0;
    let mut points = Vec::with_capacity(chars.len() + 1);
    points.push(egui::pos2(0.0, 0.0));

    while i < chars.len() {
        if chars[i] == '\n' {
            x = 0.0;
            y += line_height;
            i += 1;
            points.push(egui::pos2(x, y));
            continue;
        }

        let mut matched = false;
        for len in [2usize, 1] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if emoji_map.contains_key(&s) {
                    x += emoji_size + 2.0;
                    for _ in 0..len {
                        points.push(egui::pos2(x, y));
                    }
                    i += len;
                    matched = true;
                    break;
                }
            }
        }

        if !matched {
            let ch = chars[i].to_string();
            x += measure_text_width(ui, &ch);
            i += 1;
            points.push(egui::pos2(x, y));
        }
    }

    points
}

fn cursor_from_point(points: &[egui::Pos2], target: egui::Pos2) -> usize {
    let mut best_idx = 0;
    let mut best_dist = f32::MAX;

    for (idx, p) in points.iter().enumerate() {
        let dx = p.x - target.x;
        let dy = p.y - target.y;
        let d = dx * dx + dy * dy;
        if d < best_dist {
            best_dist = d;
            best_idx = idx;
        }
    }

    best_idx
}

fn move_cursor_vertical(
    points: &[egui::Pos2],
    cursor_char: &mut usize,
    delta_lines: i32,
    line_height: f32,
) {
    if points.is_empty() {
        return;
    }

    let current = points
        .get(*cursor_char)
        .copied()
        .unwrap_or_else(|| *points.last().unwrap_or(&egui::pos2(0.0, 0.0)));
    let current_line = (current.y / line_height).round() as i32;
    let target_line = current_line + delta_lines;
    if target_line < 0 {
        return;
    }

    let mut best_idx: Option<usize> = None;
    let mut best_dist = f32::MAX;
    for (idx, p) in points.iter().enumerate() {
        let line = (p.y / line_height).round() as i32;
        if line == target_line {
            let dist = (p.x - current.x).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(idx);
            }
        }
    }

    if let Some(idx) = best_idx {
        *cursor_char = idx;
    }
}

fn custom_composer_input(
    ui: &mut egui::Ui,
    input: &mut String,
    cursor_char: &mut usize,
    input_has_focus: &mut bool,
    scroll_lines: &mut f32,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
    emoji_alias_to_char: &std::collections::HashMap<String, String>,
    emoji_aliases: &[String],
    shortcode_menu_open: bool,
    width: f32,
) -> (egui::Response, bool, bool) {
    let line_height = 22.0;
    let line_count = input.chars().filter(|&c| c == '\n').count().saturating_add(1);
    let visual_lines = line_count.clamp(1, 10) as f32;
    let desired_size = egui::vec2(width.max(120.0), 16.0 + visual_lines * line_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let needs_scrollbar = line_count > 10;
    let content_rect = if needs_scrollbar {
        egui::Rect::from_min_max(
            rect.min + egui::vec2(6.0, 6.0),
            rect.max - egui::vec2(14.0, 6.0),
        )
    } else {
        rect.shrink2(egui::vec2(6.0, 6.0))
    };
    let caret_points = composer_caret_positions(ui, input, emoji_map, 18.0);
    let max_scroll = (line_count as f32 - visual_lines).max(0.0);
    *scroll_lines = scroll_lines.clamp(0.0, max_scroll);

    if response.clicked() {
        *input_has_focus = true;
        response.request_focus();
        if let Some(pos) = response.interact_pointer_pos() {
            let local = egui::pos2(
                (pos.x - content_rect.left()).max(0.0),
                (pos.y - content_rect.top()).max(0.0),
            );
            *cursor_char = cursor_from_point(&caret_points, local);
        } else {
            *cursor_char = input.chars().count();
        }
    }

    let has_focus = *input_has_focus || response.has_focus();
    let mut changed = false;
    let mut submit = false;
    let total_chars = input.chars().count();
    if *cursor_char > total_chars {
        *cursor_char = total_chars;
    }

    if let Some(caret) = caret_points.get(*cursor_char) {
        let caret_line = (caret.y / line_height).floor();
        if caret_line < *scroll_lines {
            *scroll_lines = caret_line;
        }
        if caret_line >= *scroll_lines + visual_lines {
            *scroll_lines = caret_line - visual_lines + 1.0;
        }
        *scroll_lines = scroll_lines.clamp(0.0, max_scroll);
    }

    if has_focus {
        let caret = caret_points
            .get(*cursor_char)
            .copied()
            .unwrap_or_else(|| *caret_points.last().unwrap_or(&egui::pos2(0.0, 0.0)));
        let cursor_x = content_rect.left() + caret.x + 1.0;
        let cursor_top = content_rect.top() + caret.y + 2.0 - (*scroll_lines * line_height);
        let cursor_bottom = (cursor_top + 18.0).min(content_rect.bottom() - 2.0);
        ui.ctx().output_mut(|o| {
            o.mutable_text_under_cursor = true;
            o.ime = Some(egui::output::IMEOutput {
                rect,
                cursor_rect: egui::Rect::from_min_max(
                    egui::pos2(cursor_x, cursor_top.max(content_rect.top())),
                    egui::pos2(cursor_x + 1.0, cursor_bottom.max(cursor_top.max(content_rect.top()))),
                ),
            });
        });

        if response.hovered() {
            let wheel_y = ui.input(|i| i.raw_scroll_delta.y + i.smooth_scroll_delta.y);
            if wheel_y.abs() > 0.0 && max_scroll > 0.0 {
                *scroll_lines = (*scroll_lines - wheel_y / 32.0).clamp(0.0, max_scroll);
            }
        }

        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(t) => {
                    if !t.contains('\n') && !t.contains('\r') {
                        if t == " " {
                            if let Some((start, query)) = emoji_shortcode_trigger(input, *cursor_char) {
                                if !query.is_empty() {
                                    if let Some(ch) = emoji_alias_to_char.get(&query) {
                                        replace_char_range(input, cursor_char, start, *cursor_char, ch);
                                        insert_text_at_cursor(input, cursor_char, " ");
                                        changed = true;
                                        continue;
                                    }
                                }
                            }
                        }
                        insert_text_at_cursor(input, cursor_char, &t);
                        changed = true;
                    }
                }
                egui::Event::Ime(egui::ImeEvent::Commit(t)) => {
                    if !t.contains('\n') && !t.contains('\r') && !t.is_empty() {
                        insert_text_at_cursor(input, cursor_char, &t);
                        changed = true;
                    }
                }
                egui::Event::Paste(t) => {
                    insert_text_at_cursor(input, cursor_char, &t.replace(['\r', '\n'], " "));
                    changed = true;
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => match key {
                    egui::Key::Enter => {
                        if modifiers.shift {
                            insert_text_at_cursor(input, cursor_char, "\n");
                            changed = true;
                        } else {
                            submit = true;
                        }
                    }
                    egui::Key::Tab => {
                        let suggestions = shortcode_suggestions(
                            input,
                            *cursor_char,
                            emoji_alias_to_char,
                            emoji_aliases,
                            1,
                        );
                        if let Some((_alias, ch)) = suggestions.first() {
                            if let Some((start, _query)) = emoji_shortcode_trigger(input, *cursor_char) {
                                replace_char_range(input, cursor_char, start, *cursor_char, ch);
                                changed = true;
                            }
                        }
                    }
                    egui::Key::Backspace => {
                        changed |= remove_prev_char(input, cursor_char);
                    }
                    egui::Key::Delete => {
                        changed |= remove_next_char(input, cursor_char);
                    }
                    egui::Key::ArrowLeft => {
                        if *cursor_char > 0 {
                            *cursor_char -= 1;
                        }
                    }
                    egui::Key::ArrowRight => {
                        let len = input.chars().count();
                        if *cursor_char < len {
                            *cursor_char += 1;
                        }
                    }
                    egui::Key::ArrowUp => {
                        if !shortcode_menu_open {
                            let points = composer_caret_positions(ui, input, emoji_map, 18.0);
                            move_cursor_vertical(&points, cursor_char, -1, line_height);
                        }
                    }
                    egui::Key::ArrowDown => {
                        if !shortcode_menu_open {
                            let points = composer_caret_positions(ui, input, emoji_map, 18.0);
                            move_cursor_vertical(&points, cursor_char, 1, line_height);
                        }
                    }
                    egui::Key::Home => {
                        *cursor_char = 0;
                    }
                    egui::Key::End => {
                        *cursor_char = input.chars().count();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        ui.visuals().extreme_bg_color,
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Outside,
    );

    if input.is_empty() {
        ui.painter().text(
            content_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            "Message",
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().weak_text_color(),
        );
    } else {
        let painter = ui.painter().with_clip_rect(content_rect);
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;
        let mut x = content_rect.left();
        let scroll_px = *scroll_lines * line_height;
        let mut y = content_rect.top() + 11.0 - scroll_px;

        while i < chars.len() {
            if chars[i] == '\n' {
                x = content_rect.left();
                y += line_height;
                i += 1;
                continue;
            }

            let mut matched = false;
            for len in [2usize, 1] {
                if i + len <= chars.len() {
                    let s: String = chars[i..i + len].iter().collect();
                    if let Some(&idx) = emoji_map.get(&s) {
                        if let Some((_, tex)) = emoji_textures.get(idx) {
                            let img_rect = egui::Rect::from_min_size(
                                egui::pos2(x, y - 9.0),
                                egui::vec2(18.0, 18.0),
                            );
                            painter.image(
                                tex.id(),
                                img_rect,
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                            x += 20.0;
                        }
                        i += len;
                        matched = true;
                        break;
                    }
                }
            }

            if !matched {
                let run_start = i;
                while i < chars.len() {
                    if chars[i] == '\n' {
                        break;
                    }
                    let mut next_is_emoji = false;
                    for len in [2usize, 1] {
                        if i + len <= chars.len() {
                            let s: String = chars[i..i + len].iter().collect();
                            if emoji_map.contains_key(&s) {
                                next_is_emoji = true;
                                break;
                            }
                        }
                    }
                    if next_is_emoji {
                        break;
                    }
                    i += 1;
                }

                let run: String = chars[run_start..i].iter().collect();
                painter.text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    &run,
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
                x += measure_text_width(ui, &run);
            }
        }

        if needs_scrollbar {
            let track = egui::Rect::from_min_max(
                egui::pos2(rect.right() - 8.0, content_rect.top()),
                egui::pos2(rect.right() - 4.0, content_rect.bottom()),
            );
            ui.painter()
                .rect_filled(track, 2.0, ui.visuals().widgets.noninteractive.bg_fill);

            let thumb_h = (track.height() * (visual_lines / line_count as f32)).max(18.0);
            let travel = (track.height() - thumb_h).max(0.0);
            let t = if max_scroll <= 0.0 { 0.0 } else { *scroll_lines / max_scroll };
            let thumb_top = track.top() + travel * t;
            let thumb = egui::Rect::from_min_max(
                egui::pos2(track.left(), thumb_top),
                egui::pos2(track.right(), thumb_top + thumb_h),
            );
            ui.painter().rect_filled(
                thumb,
                2.0,
                ui.visuals().widgets.active.bg_fill.gamma_multiply(0.9),
            );

            let scroll_id = response.id.with("scrollbar");
            let scroll_resp = ui.interact(track, scroll_id, egui::Sense::click_and_drag());
            if (scroll_resp.clicked() || scroll_resp.dragged()) && max_scroll > 0.0 {
                if let Some(pos) = scroll_resp.interact_pointer_pos() {
                    let rel = ((pos.y - track.top()) / track.height()).clamp(0.0, 1.0);
                    *scroll_lines = rel * max_scroll;
                }
            }
        }
    }

    if has_focus {
        let blink_on = ((ui.input(|i| i.time) * 2.0) as i64) % 2 == 0;
        if blink_on {
            let caret = caret_points
                .get(*cursor_char)
                .copied()
                .unwrap_or_else(|| *caret_points.last().unwrap_or(&egui::pos2(0.0, 0.0)));
            let x = content_rect.left() + caret.x + 1.0;
            let top = content_rect.top() + caret.y + 2.0 - (*scroll_lines * line_height);
            let bottom = (top + 18.0).min(content_rect.bottom() - 2.0);
            if top < content_rect.bottom() && bottom > content_rect.top() {
                ui.painter().line_segment(
                    [egui::pos2(x, top.max(content_rect.top())), egui::pos2(x, bottom)],
                    egui::Stroke::new(1.6, ui.visuals().text_color()),
                );
            }
        }
    }

    (response, submit, changed)
}

fn play_notification_sound() {
    std::thread::spawn(|| {
        use rodio::source::Source;
        use std::time::Duration;

        let (stream, stream_handle) = match rodio::OutputStream::try_default() {
            Ok(s) => s,
            Err(e) => { eprintln!("[son] Erreur ouverture sortie audio: {}", e); return; }
        };
        let sink = match rodio::Sink::try_new(&stream_handle) {
            Ok(s) => s,
            Err(e) => { eprintln!("[son] Erreur création sink: {}", e); return; }
        };

        let tone1 = rodio::source::SineWave::new(880.0)
            .take_duration(Duration::from_millis(80))
            .amplify(0.25);
        let tone2 = rodio::source::SineWave::new(1100.0)
            .take_duration(Duration::from_millis(80))
            .amplify(0.20);

        sink.append(tone1);
        sink.append(tone2);
        sink.sleep_until_end();
        drop(stream);
    });
}

impl AbcomApp {
    fn new(
        state: Arc<Mutex<AppState>>,
        event_rx: mpsc::Receiver<AppEvent>,
        send_tx: mpsc::Sender<SendRequest>,
        send_group_tx: mpsc::Sender<SendGroupRequest>,
        typing_tx: mpsc::Sender<SendTypingRequest>,
        transfer_cmd_tx: mpsc::Sender<TransferCommand>,
    ) -> Self {
        Self {
            state,
            event_rx,
            send_tx,
            send_group_tx,
            transfer_cmd_tx,
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
            typing_tx,
            last_typing_sent: std::time::Instant::now() - Duration::from_secs(10),
            muted_conversations: std::collections::HashSet::new(),
            selected_network_filter: None, // sera initialisé au premier update
            active_view: AppView::Chat,
            networks_view_selected: None,
            editing_network_alias: None,
            editing_peer_alias: None,
            share_selection: Vec::new(),
            show_share_modal: false,
            share_error: None,
        }
    }
}

impl eframe::App for AbcomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Capturer l'état du focus au début (avant le traitement des événements)
        self.window_focused = ctx.input(|i| i.focused);

        // Chargement paresseux des textures emoji (nécessite le contexte egui)
        if !self.emoji_textures_loaded {
            self.emoji_textures = load_emoji_textures(ctx);
            self.emoji_map = self.emoji_textures
                .iter()
                .enumerate()
                .map(|(i, (ch, _))| (ch.clone(), i))
                .collect();
            let available: Vec<String> = self
                .emoji_textures
                .iter()
                .map(|(ch, _)| ch.clone())
                .collect();
            let (alias_to_char, aliases) = build_emoji_shortcode_index(&available);
            self.emoji_alias_to_char = alias_to_char;
            self.emoji_aliases = aliases;
            self.emoji_textures_loaded = true;
        }

        // Dépiler les événements réseau reçus depuis les tâches tokio
        {
            let mut s = self.state.lock().unwrap();
            while let Ok(evt) = self.event_rx.try_recv() {
                match evt {
                    AppEvent::MessageReceived(msg) => {
                        s.add_message(msg.clone());
                        if msg.from != s.my_username {
                            // Notification visuelle dans l'app
                            self.last_notification = Some(format!("{}: {}", msg.from, msg.content));
                            self.notification_time = std::time::Instant::now();
                            self.has_unread = true;
                            // Déterminer le salon source du message
                            let source_conv: Option<String> = match &msg.to_user {
                                None => None,
                                Some(target) if target.starts_with('#') => Some(target.clone()),
                                Some(_) => Some(msg.from.clone()),
                            };
                            // Son si : on n'est pas dans ce salon ET ce salon n'est pas muet
                            let already_in_conv = s.selected_conversation == source_conv;
                            let conv_muted = self.muted_conversations.contains(&source_conv);
                            if self.enable_sound_notifications && !already_in_conv && !conv_muted {
                                play_notification_sound();
                            }
                        }
                    }
                    AppEvent::PeerDiscovered { username, addr } => s.add_peer(username, addr),
                    AppEvent::PeerDisconnected { username } => {
                        // Marquer le pair hors ligne (sans supprimer l'historique ni la carte)
                        if let Some(peer) = s.peers.iter_mut().find(|p| p.username == username) {
                            peer.online = false;
                        }
                    }
                    AppEvent::UserTyping(username) => s.set_user_typing(username),
                    AppEvent::UserStoppedTyping(_username) => {
                        s.clear_typing_if_old();
                    }
                    AppEvent::GroupEventReceived(evt) => {
                        // Traiter les événements de synchronisation de groupe
                        use crate::message::GroupAction;
                        
                        match evt.action {
                            GroupAction::Create { group } => {
                                // Ajouter le groupe s'il n'existe pas déjà
                                if !s.groups.iter().any(|g| g.name == group.name) {
                                    s.groups.push(group);
                                    s.save_groups();
                                    eprintln!("[ui] Groupe reçu et ajouté: {}", s.groups.last().map(|g| &g.name).unwrap_or(&"".to_string()));
                                }
                            }
                            GroupAction::AddMember { group_name, username } => {
                                if let Some(group) = s.groups.iter_mut().find(|g| g.name == group_name) {
                                    if !group.members.contains(&username) {
                                        group.members.push(username);
                                        s.save_groups();
                                    }
                                }
                            }
                            GroupAction::RemoveMember { group_name, username } => {
                                if let Some(group) = s.groups.iter_mut().find(|g| g.name == group_name) {
                                    group.members.retain(|m| m != &username);
                                    s.save_groups();
                                }
                            }
                            GroupAction::Rename { group_name, new_name } => {
                                if let Some(group) = s.groups.iter_mut().find(|g| g.name == group_name) {
                                    group.name = new_name;
                                    s.save_groups();
                                }
                            }
                            GroupAction::Delete { group_name } => {
                                s.groups.retain(|g| g.name != group_name);
                                s.save_groups();
                            }
                        }
                    }
                    AppEvent::Transfer(evt) => s.apply_transfer_event(evt),
                }
            }
            s.clear_typing_if_old();
        }

        // Nettoyer les pairs inactifs toutes les 5 secondes (timeout: 10 secondes, synchronisé avec UDP discovery)
        if self.last_cleanup_time.elapsed().as_secs() >= 5 {
            self.last_cleanup_time = std::time::Instant::now();
            {
                let mut s = self.state.lock().unwrap();
                let _disconnected = s.cleanup_inactive_peers(10);
                // Les pairs sont marqués offline automatiquement, la UI se mettra à jour
            }
            // Re-détecter le réseau actif toutes les 15s (SSID + subnet)
            // Exécuté ici car peut invoquer des commandes système (iwgetid, nmcli)
            if self.last_network_check.elapsed().as_secs() >= 15 {
                self.last_network_check = std::time::Instant::now();
                let (new_id, new_subnet) = crate::app::AppState::detect_network_id();
                let (old_id, old_subnet) = {
                    let s = self.state.lock().unwrap();
                    (s.current_network_id.clone(), s.current_subnet.clone())
                };
                if new_id != old_id || new_subnet != old_subnet {
                    let mut s = self.state.lock().unwrap();
                    s.current_network_id = new_id.clone();
                    s.current_subnet = new_subnet.clone();
                    if let Some(ref id) = new_id {
                        let sn = new_subnet.as_deref();
                        s.ensure_network_known(id, sn);
                    }
                    drop(s);
                    // Basculer le filtre sur le nouveau réseau automatiquement
                    self.selected_network_filter = new_id;
                }
            }
        }

        // Repeindre toutes les 100 ms pour capter les nouveaux messages
        ctx.request_repaint_after(Duration::from_millis(100));

        // Flash de la barre des tâches si message non lu et fenêtre non focalisée
        if self.has_unread {
            let focused = ctx.input(|i| i.focused);
            if !focused {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Informational,
                ));
            } else {
                // La fenêtre est revenue au premier plan : effacer le flag
                self.has_unread = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                    egui::UserAttentionType::Reset,
                ));
            }
        }

        // ── Panneau gauche : conversations et salons ──────────────────────
        egui::SidePanel::left("peers_panel")
            .resizable(false)
            .exact_width(220.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);

                // Initialiser le filtre réseau sur le réseau actuel si pas encore fait
                let (current_network_id, known_networks, peers_all, selected_conv, unread_counts_all, peer_records) = {
                    let s = self.state.lock().unwrap();
                    let peers = s.peers.clone();
                    let unread_counts = peers.iter().map(|p| s.unread_count(&p.username)).collect::<Vec<_>>();
                    (s.current_network_id.clone(), s.known_networks.clone(), peers, s.selected_conversation.clone(), unread_counts, s.peer_records.clone())
                };
                if self.selected_network_filter.is_none() {
                    self.selected_network_filter = current_network_id.clone();
                }

                // ── Sélecteur de réseau ──
                ui.horizontal(|ui| {
                    ui.label("🌐");
                    let current_label = self.selected_network_filter.as_ref()
                        .and_then(|s| known_networks.iter().find(|n| &n.id == s))
                        .map(|n| n.display_name())
                        .unwrap_or_else(|| "Tous".to_string());
                    egui::ComboBox::from_id_salt("network_filter")
                        .selected_text(&current_label)
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            // Option "Tous les réseaux"
                            if ui.selectable_label(self.selected_network_filter.is_none(), "🌐 Tous les réseaux").clicked() {
                                self.selected_network_filter = None;
                            }
                            for net in &known_networks {
                                let is_selected = self.selected_network_filter.as_ref() == Some(&net.id);
                                let is_current = current_network_id.as_ref() == Some(&net.id);
                                let label = if is_current {
                                    format!("📡 {} (actuel)", net.display_name())
                                } else {
                                    format!("🔌 {}", net.display_name())
                                };
                                if ui.selectable_label(is_selected, label).clicked() {
                                    self.selected_network_filter = Some(net.id.clone());
                                }
                            }
                        });
                });

                // Filtrer les pairs selon le réseau sélectionné
                let (peers, unread_counts): (Vec<_>, Vec<_>) = if let Some(ref network_id) = self.selected_network_filter {
                    let seen: Vec<&str> = known_networks.iter()
                        .find(|n| &n.id == network_id)
                        .map(|n| n.seen_peers.iter().map(|s| s.as_str()).collect())
                        .unwrap_or_default();
                    peers_all.iter().zip(unread_counts_all.iter())
                        .filter(|(p, _)| seen.contains(&p.username.as_str()))
                        .map(|(p, u)| (p.clone(), *u))
                        .unzip()
                } else {
                    (peers_all.clone(), unread_counts_all.clone())
                };

                // Section: Conversations privées
                ui.heading("👥 Conversations");
                ui.add_space(4.0);
                if peers.is_empty() {
                    ui.weak("En attente de pairs...");
                } else {
                    for (idx, peer) in peers.iter().enumerate() {
                        let is_selected = selected_conv.as_ref().map(|c| c == &peer.username).unwrap_or(false);
                        let unread = unread_counts[idx];

                        let desired = egui::vec2(ui.available_width(), 56.0);
                        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
                        let visuals = ui.style().interact(&resp);
                        let fill = if is_selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            visuals.bg_fill
                        };
                        let stroke = if is_selected {
                            ui.visuals().selection.stroke
                        } else {
                            visuals.bg_stroke
                        };

                        ui.painter().rect_filled(rect, 8.0, fill);
                        ui.painter().rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Outside);

                        // Diode de statut (verte = en ligne, rouge = hors ligne)
                        let dot_radius = 5.0;
                        let dot_center = egui::pos2(rect.left() + 10.0, rect.center().y);
                        let dot_color = if peer.online {
                            egui::Color32::from_rgb(50, 200, 80)
                        } else {
                            egui::Color32::from_rgb(180, 40, 40)
                        };
                        ui.painter().circle_filled(dot_center, dot_radius, dot_color);

                        let text_color = ui.visuals().text_color();
                        let font_id = egui::TextStyle::Button.resolve(ui.style());
                        let text_pos = rect.left_center() + egui::vec2(24.0, 0.0);
                        // Afficher l'alias s'il existe, sinon le username
                        let display_name = peer_records.iter()
                            .find(|r| r.username == peer.username)
                            .and_then(|r| r.alias.clone())
                            .unwrap_or_else(|| peer.username.clone());
                        ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, &display_name, font_id.clone(), text_color);

                        if unread > 0 {
                            let badge_text = if unread > 99 {
                                "99+".to_string()
                            } else {
                                unread.to_string()
                            };
                            let badge_size = 24.0;
                            let badge_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.right() - badge_size - 12.0, rect.center().y - badge_size / 2.0),
                                egui::vec2(badge_size, badge_size),
                            );

                            ui.painter().rect_filled(badge_rect, badge_size / 2.0, egui::Color32::from_rgb(220, 40, 60));
                            ui.painter().text(
                                badge_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                badge_text,
                                egui::TextStyle::Body.resolve(ui.style()),
                                egui::Color32::WHITE,
                            );
                        }

                        if resp.clicked() {
                            let mut s = self.state.lock().unwrap();
                            if is_selected {
                                s.selected_conversation = None;
                            } else {
                                s.selected_conversation = Some(peer.username.clone());
                                s.mark_conversation_read(&peer.username);
                            }
                        }

                        ui.add_space(4.0);
                    }
                }

                ui.separator();
                ui.add_space(4.0);
                // Section: Groupes
                ui.horizontal(|ui| {
                    ui.heading("🔗 Groupes");
                    if ui.small_button("+").clicked() {
                        self.show_group_modal = true;
                        self.group_name_input.clear();
                        self.group_members_selected.clear();
                    }
                });
                ui.add_space(4.0);

                // Afficher les groupes
                let groups = self.state.lock().unwrap().groups.clone();
                if groups.is_empty() {
                    ui.weak("Aucun groupe");
                } else {
                    for group in &groups {
                        let is_selected = selected_conv.as_ref().map(|c| c == &format!("#{}", group.name)).unwrap_or(false);
                        let desired = egui::vec2(ui.available_width(), 56.0);
                        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
                        let visuals = ui.style().interact(&resp);
                        let fill = if is_selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            visuals.bg_fill
                        };
                        let stroke = if is_selected {
                            ui.visuals().selection.stroke
                        } else {
                            visuals.bg_stroke
                        };

                        ui.painter().rect_filled(rect, 8.0, fill);
                        ui.painter().rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Outside);

                        // Icône de groupe
                        let text_color = ui.visuals().text_color();
                        let font_id = egui::TextStyle::Button.resolve(ui.style());
                        let text_pos = rect.left_center() + egui::vec2(10.0, 0.0);
                        ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, &format!("🔗 {}", group.name), font_id.clone(), text_color);

                        if resp.clicked() {
                            let mut s = self.state.lock().unwrap();
                            s.selected_conversation = Some(format!("#{}", group.name));
                        }

                        ui.add_space(4.0);
                    }
                }

                // Global conversation
                let is_global_selected = selected_conv.is_none() && self.active_view == AppView::Chat;
                let resp = ui.add_sized(
                    [ui.available_width(), 56.0],
                    egui::SelectableLabel::new(is_global_selected, "📢 Tous"),
                );
                if resp.clicked() {
                    self.state.lock().unwrap().selected_conversation = None;
                    self.active_view = AppView::Chat;
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let my_name = self.state.lock().unwrap().my_username.clone();
                    ui.separator();
                    ui.label(egui::RichText::new(format!("Vous : {}", my_name)).small());
                    ui.add_space(4.0);
                    let networks_btn = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::SelectableLabel::new(self.active_view == AppView::Networks, "🌐  Gérer les réseaux"),
                    );
                    if networks_btn.clicked() {
                        self.active_view = if self.active_view == AppView::Networks {
                            AppView::Chat
                        } else {
                            AppView::Networks
                        };
                    }
                });
            });

        // ── Panneau typage : indicateurs de frappe ─────────────────────────
        let typing_list = self.state.lock().unwrap().typing_users_list();
        if !typing_list.is_empty() {
            let typing_text = format!("✍ {} en train d'écrire...", typing_list.join(", "));
            egui::TopBottomPanel::bottom("typing_panel")
                .exact_height(25.0)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(typing_text).weak().small());
                });
        }

        // ── Barre du bas : champ de saisie ────────────────────────────────
        // Cacher la saisie si la conversation sélectionnée est un pair hors ligne
        let selected_peer_online = {
            let s = self.state.lock().unwrap();
            match &s.selected_conversation {
                None => true, // Global = toujours actif
                Some(conversation) if conversation.starts_with('#') => true,
                Some(username) => s.is_peer_online(username),
            }
        };
        let mut emoji_button_clicked = false;

        if !selected_peer_online {
            egui::TopBottomPanel::bottom("input_panel")
                .exact_height(40.0)
                .show(ctx, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("🔴 Cet utilisateur est hors ligne")
                                .color(egui::Color32::from_rgb(180, 40, 40))
                                .small(),
                        );
                    });
                });
        } else {
        let composer_line_count = self.input.chars().filter(|&c| c == '\n').count().saturating_add(1);
        let composer_visible_lines = composer_line_count.clamp(1, 10) as f32;
        let composer_height = 16.0 + composer_visible_lines * 22.0;

        egui::TopBottomPanel::bottom("input_panel")
            .exact_height((composer_height + 20.0).max(68.0))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);

                    // ─── Colonne d'actions (emoji + partage) ───
                    let (emoji_btn_response, share_btn_response) = ui
                        .allocate_ui_with_layout(
                            egui::vec2(28.0, composer_height),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                let emoji_resp = if !self.emoji_textures.is_empty() {
                                    let (_ch, tex) = &self.emoji_textures[0];
                                    let (btn_rect, btn_resp) = ui.allocate_exact_size(
                                        egui::vec2(24.0, 24.0),
                                        egui::Sense::click(),
                                    );
                                    if self.show_emoji_picker {
                                        ui.painter().rect_filled(btn_rect, 6.0, ui.visuals().selection.bg_fill);
                                    }
                                    let img_rect = btn_rect.shrink(3.0);
                                    ui.painter().image(
                                        tex.id(),
                                        img_rect,
                                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE,
                                    );
                                    btn_resp
                                } else {
                                    ui.button("😊")
                                };

                                ui.add_space(6.0);
                                let share_resp = ui
                                    .add_sized([24.0, 24.0], egui::Button::new("📎"))
                                    .on_hover_text("Partager des fichiers ou des dossiers");

                                (emoji_resp, share_resp)
                            },
                        )
                        .inner;
                    if emoji_btn_response.clicked() {
                        self.show_emoji_picker = !self.show_emoji_picker;
                        emoji_button_clicked = true;
                    }
                    if share_btn_response.clicked() {
                        self.show_share_modal = true;
                        self.share_error = None;
                    }

                    ui.add_space(4.0);

                    // ─── Zone de saisie (prend toute la place disponible) ───
                    let (selected_addr, all_peers, group_addrs) = {
                        let s = self.state.lock().unwrap();
                        let group_targets = match &s.selected_conversation {
                            Some(conversation) if conversation.starts_with('#') => {
                                let group_name = conversation.trim_start_matches('#');
                                s.groups
                                    .iter()
                                    .find(|group| group.name == group_name)
                                    .map(|group| {
                                        group
                                            .members
                                            .iter()
                                            .filter(|member| *member != &s.my_username)
                                            .filter_map(|member| {
                                                s.peers
                                                    .iter()
                                                    .find(|peer| peer.online && peer.username == *member)
                                                    .map(|peer| peer.addr)
                                            })
                                            .collect::<Vec<_>>()
                                    })
                                    .unwrap_or_default()
                            }
                            _ => Vec::new(),
                        };
                        (s.selected_peer_addr(), s.peers.clone(), group_targets)
                    };

                    let available_w = ui.available_width() - 8.0;
                    let menu_open_now = emoji_shortcode_trigger(&self.input, self.input_cursor_char)
                        .map(|(_, q)| !q.is_empty())
                        .unwrap_or(false);

                    let (resp, mut pressed_enter, changed) = custom_composer_input(
                        ui,
                        &mut self.input,
                        &mut self.input_cursor_char,
                        &mut self.input_has_focus,
                        &mut self.input_scroll_lines,
                        &self.emoji_map,
                        &self.emoji_textures,
                        &self.emoji_alias_to_char,
                        &self.emoji_aliases,
                        menu_open_now,
                        available_w - 12.0,
                    );

                    let shortcode_limit = match emoji_shortcode_trigger(&self.input, self.input_cursor_char) {
                        Some((_start, query)) if query.is_empty() => 5,
                        _ => 12,
                    };

                    let shortcode_list = shortcode_suggestions(
                        &self.input,
                        self.input_cursor_char,
                        &self.emoji_alias_to_char,
                        &self.emoji_aliases,
                        shortcode_limit,
                    );
                    let mut clicked_shortcode: Option<String> = None;
                    if shortcode_list.is_empty() {
                        self.shortcode_selected = 0;
                    } else if self.shortcode_selected >= shortcode_list.len() {
                        self.shortcode_selected = shortcode_list.len() - 1;
                    }

                    if self.input_has_focus && !shortcode_list.is_empty() {
                        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                            self.shortcode_selected = (self.shortcode_selected + 1).min(shortcode_list.len() - 1);
                        }
                        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                            self.shortcode_selected = self.shortcode_selected.saturating_sub(1);
                        }
                    }

                    if self.input_has_focus && !shortcode_list.is_empty() {
                        let row_h = 28.0;
                        let desired_h = (shortcode_list.len() as f32 * row_h + 8.0).min(220.0);
                        let screen = ctx.screen_rect();
                        let gap = 14.0;
                        let popup_bottom = resp.rect.top() - gap;
                        let available_above = (popup_bottom - (screen.top() + 4.0)).max(0.0);
                        let popup_h = desired_h.min(available_above);
                        if popup_h > 8.0 {
                            let popup_w = resp.rect.width().max(260.0);
                            let popup_y = popup_bottom - popup_h;
                            let popup_pos = egui::pos2(resp.rect.left(), popup_y);
                            egui::Area::new("emoji_shortcode_popup".into())
                                .order(egui::Order::Foreground)
                                .fixed_pos(popup_pos)
                                .show(ctx, |ui| {
                                    ui.set_min_size(egui::vec2(popup_w, popup_h));
                                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.set_min_width(popup_w);
                                        ui.set_min_height(popup_h);
                                        egui::ScrollArea::vertical()
                                            .auto_shrink([false, false])
                                            .max_height((popup_h - 8.0).max(row_h))
                                            .show_rows(ui, row_h, shortcode_list.len(), |ui, row_range| {
                                                for idx in row_range {
                                                    let (alias, ch) = &shortcode_list[idx];
                                                    let row_w = ui.available_width().max(popup_w - 16.0);
                                                    let (row_rect, row_resp) = ui.allocate_exact_size(
                                                        egui::vec2(row_w, row_h),
                                                        egui::Sense::click(),
                                                    );

                                                    let hovered = row_resp.hovered();
                                                    let selected = idx == self.shortcode_selected;
                                                    if hovered || selected {
                                                        let fill = if hovered {
                                                            ui.visuals().widgets.hovered.bg_fill
                                                        } else {
                                                            ui.visuals().widgets.active.bg_fill.gamma_multiply(0.6)
                                                        };
                                                        ui.painter().rect_filled(row_rect, 4.0, fill);
                                                    }

                                                    let mut x = row_rect.left() + 8.0;
                                                    let y = row_rect.center().y;
                                                    if let Some(&tex_idx) = self.emoji_map.get(ch) {
                                                        if let Some((_, tex)) = self.emoji_textures.get(tex_idx) {
                                                            let img_rect = egui::Rect::from_center_size(
                                                                egui::pos2(x + 9.0, y),
                                                                egui::vec2(18.0, 18.0),
                                                            );
                                                            ui.painter().image(
                                                                tex.id(),
                                                                img_rect,
                                                                egui::Rect::from_min_max(
                                                                    egui::pos2(0.0, 0.0),
                                                                    egui::pos2(1.0, 1.0),
                                                                ),
                                                                egui::Color32::WHITE,
                                                            );
                                                        }
                                                    }
                                                    x += 26.0;

                                                    let label = format!(":{}", alias);
                                                    let text_color = if selected {
                                                        ui.visuals().strong_text_color()
                                                    } else {
                                                        ui.visuals().text_color()
                                                    };
                                                    ui.painter().text(
                                                        egui::pos2(x, y),
                                                        egui::Align2::LEFT_CENTER,
                                                        label,
                                                        egui::TextStyle::Body.resolve(ui.style()),
                                                        text_color,
                                                    );

                                                    if row_resp.clicked() {
                                                        self.shortcode_selected = idx;
                                                        clicked_shortcode = Some(alias.clone());
                                                    }
                                                }
                                            });
                                    });
                                });
                        }
                    }

                    if self.input_has_focus && !shortcode_list.is_empty() && pressed_enter {
                        if let Some((alias, _ch)) = shortcode_list.get(self.shortcode_selected) {
                            clicked_shortcode = Some(alias.clone());
                            pressed_enter = false;
                        }
                    }

                    if let Some(alias) = clicked_shortcode {
                        if let Some((start, _query)) =
                            emoji_shortcode_trigger(&self.input, self.input_cursor_char)
                        {
                            if let Some(ch) = self.emoji_alias_to_char.get(&alias) {
                                let end = self.input_cursor_char;
                                replace_char_range(
                                    &mut self.input,
                                    &mut self.input_cursor_char,
                                    start,
                                    end,
                                    ch,
                                );
                                self.input_has_focus = true;
                                self.show_emoji_picker = false;
                            }
                        }
                    }

                    // Détecter la frappe et envoyer l'indicateur (max 1 fois/1.5s).
                    // - Conversation directe → uniquement ce pair
                    // - Groupe → tous les membres en ligne
                    // - Broadcast global (None) → tous les pairs en ligne
                    if changed && self.last_typing_sent.elapsed().as_millis() > 1500 {
                        self.last_typing_sent = std::time::Instant::now();
                        let (my_name, target_addrs) = {
                            let s = self.state.lock().unwrap();
                            let name = s.my_username.clone();
                            let addrs = match &s.selected_conversation {
                                None => {
                                    // Global : tous les pairs en ligne
                                    s.peers.iter()
                                        .filter(|p| p.online)
                                        .map(|p| p.addr)
                                        .collect::<Vec<_>>()
                                }
                                Some(conv) => {
                                    if conv.starts_with('#') {
                                        let group_name = conv.trim_start_matches('#');
                                        s.groups
                                            .iter()
                                            .find(|group| group.name == group_name)
                                            .map(|group| {
                                                group
                                                    .members
                                                    .iter()
                                                    .filter(|member| *member != &s.my_username)
                                                    .filter_map(|member| {
                                                        s.peers
                                                            .iter()
                                                            .find(|peer| peer.online && peer.username == *member)
                                                            .map(|peer| peer.addr)
                                                    })
                                                    .collect::<Vec<_>>()
                                            })
                                            .unwrap_or_default()
                                    } else {
                                        s.peers.iter()
                                            .find(|p| p.online && &p.username == conv)
                                            .map(|p| p.addr)
                                            .into_iter()
                                            .collect::<Vec<_>>()
                                    }
                                }
                            };
                            (name, addrs)
                        };
                        for addr in target_addrs {
                            let _ = self.typing_tx.try_send(SendTypingRequest {
                                to_addr: addr,
                                from: my_name.clone(),
                            });
                        }
                    }

                    ui.add_space(4.0);

                    if pressed_enter && !self.input.trim().is_empty() {
                        if self.input.ends_with('\n') {
                            self.input.pop();
                        }
                        let content = self.input.trim().to_string();
                        let now = chrono::Local::now().format("%H:%M").to_string();
                        let (my_name, selected_peer_name) = {
                            let s = self.state.lock().unwrap();
                            let my_username = s.my_username.clone();
                            let peer_name = s.selected_conversation.clone();
                            (my_username, peer_name)
                        };
                        
                        let msg = ChatMessage { 
                            from: my_name, 
                            content, 
                            timestamp: now,
                            to_user: selected_peer_name.clone(),
                        };

                        self.state.lock().unwrap().add_message(msg.clone());
                        if let Some(peer_name) = &selected_peer_name {
                            self.state.lock().unwrap().selected_conversation = Some(peer_name.clone());
                        }
                        self.input.clear();
                        self.input_cursor_char = 0;
                        self.input_has_focus = true;
                        self.input_scroll_lines = 0.0;

                        if let Some(conversation) = &selected_peer_name {
                            if conversation.starts_with('#') {
                                if group_addrs.is_empty() {
                                    self.last_notification = Some("Aucun membre du groupe n'est en ligne".to_string());
                                    self.notification_time = std::time::Instant::now();
                                } else {
                                    for addr in &group_addrs {
                                        let _ = self.send_tx.try_send(SendRequest {
                                            to_addr: *addr,
                                            message: msg.clone(),
                                        });
                                    }
                                }
                            } else if let Some(addr) = selected_addr {
                                let _ = self.send_tx.try_send(SendRequest { to_addr: addr, message: msg });
                            }
                        } else {
                            for peer in &all_peers {
                                let _ = self.send_tx.try_send(SendRequest {
                                    to_addr: peer.addr,
                                    message: msg.clone(),
                                });
                            }
                        }

                        resp.request_focus();
                        self.show_emoji_picker = false;
                    }
                });

            });
        } // fin else selected_peer_online

        if self.show_share_modal {
            let selected_target = {
                let s = self.state.lock().unwrap();
                s.selected_conversation.clone()
            };

            let mut open = true;
            egui::Window::new("Partager")
                .fixed_size([520.0, 380.0])
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(match &selected_target {
                            Some(target) => format!("Destination : {}", target),
                            None => "Destination : choisissez un pair ou un groupe".to_string(),
                        })
                        .strong(),
                    );
                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("📄 Ajouter des fichiers").clicked() {
                            if let Some(paths) = rfd::FileDialog::new().set_title("Choisir des fichiers").pick_files() {
                                push_unique_paths(&mut self.share_selection, paths);
                                self.share_error = None;
                            }
                        }

                        if ui.button("📁 Ajouter un dossier").clicked() {
                            if let Some(path) = rfd::FileDialog::new().set_title("Choisir un dossier").pick_folder() {
                                push_unique_paths(&mut self.share_selection, [path]);
                                self.share_error = None;
                            }
                        }

                        if ui.button("🧹 Vider").clicked() {
                            self.share_selection.clear();
                            self.share_error = None;
                        }
                    });

                    ui.add_space(8.0);
                    ui.label(format!("{} élément(s) prêt(s)", self.share_selection.len()));
                    ui.add_space(4.0);

                    egui::ScrollArea::vertical().max_height(190.0).show(ui, |ui| {
                        if self.share_selection.is_empty() {
                            ui.weak("Ajoutez des fichiers ou un dossier à partager.");
                        }

                        let mut remove_index = None;
                        for (index, path) in self.share_selection.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let icon = if path.is_dir() { "📁" } else { "📄" };
                                ui.label(icon);
                                ui.label(path.display().to_string());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("✕").clicked() {
                                        remove_index = Some(index);
                                    }
                                });
                            });
                        }

                        if let Some(index) = remove_index {
                            self.share_selection.remove(index);
                        }
                    });

                    if let Some(error) = &self.share_error {
                        ui.add_space(6.0);
                        ui.colored_label(egui::Color32::from_rgb(210, 70, 70), error);
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Envoyer").clicked() {
                            let plan = {
                                let s = self.state.lock().unwrap();
                                s.build_transfer_request(self.share_selection.clone())
                            };

                            match plan {
                                Ok((request, skipped)) => {
                                    if self.transfer_cmd_tx.try_send(TransferCommand::QueueTransfer(request)).is_ok() {
                                        self.show_share_modal = false;
                                        self.share_error = None;
                                        self.share_selection.clear();
                                        self.last_notification = Some(if skipped.is_empty() {
                                            "Transfert lancé".to_string()
                                        } else {
                                            format!("Transfert lancé. Hors ligne : {}", skipped.join(", "))
                                        });
                                        self.notification_time = std::time::Instant::now();
                                    } else {
                                        self.share_error = Some("La file de transfert est occupée, réessayez.".to_string());
                                    }
                                }
                                Err(error) => {
                                    self.share_error = Some(error);
                                }
                            }
                        }

                        if ui.button("Fermer").clicked() {
                            self.show_share_modal = false;
                        }
                    });
                });

            if !open {
                self.show_share_modal = false;
            }
        }

        // ── Notification popup ─────────────────────────────────────────────
        if let Some(notif) = &self.last_notification {
            let elapsed = self.notification_time.elapsed().as_secs_f32();
            if elapsed < 3.0 {
                // Affichage haut droite
                egui::Window::new("Notification")
                    .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
                    .resizable(false)
                    .collapsible(false)
                    .title_bar(false)
                    .show(ctx, |ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 200, 100),
                            egui::RichText::new(notif).text_style(egui::TextStyle::Body),
                        );
                    });
            } else {
                self.last_notification = None;
            }
        }

        // ── Popup : Picker d'emojis avec catégories ──────────────────────
        if self.show_emoji_picker {
            let picker_window = egui::Window::new("Emojis")
                .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(0.0, -60.0))
                .resizable(false)
                .collapsible(false)
                .fixed_size([310.0, 340.0]);

            let mut picker_rect: Option<egui::Rect> = None;
            if let Some(window_resp) = picker_window
                .show(ctx, |ui| {
                    // Ligne d'icônes de catégories
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (cat_idx, (cat_icon, _start, _end)) in
                            crate::emoji_registry::EMOJI_CATEGORIES.iter().enumerate()
                        {
                            let selected = self.emoji_category == cat_idx;
                            let btn = egui::Button::new(
                                egui::RichText::new(*cat_icon).size(18.0)
                            )
                            .min_size(egui::vec2(24.0, 24.0))
                            .selected(selected)
                            .frame(selected);
                            if ui.add(btn).clicked() {
                                self.emoji_category = cat_idx;
                            }
                        }
                    });
                    ui.separator();

                    // Grille d'emojis — hauteur fixe
                    let (_, start, end) =
                        crate::emoji_registry::EMOJI_CATEGORIES[self.emoji_category];
                    let slice = &self.emoji_textures[start..end.min(self.emoji_textures.len())];

                    egui::ScrollArea::vertical()
                        .max_height(270.0)
                        .min_scrolled_height(270.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Grid::new("emoji_grid")
                                .spacing([3.0, 3.0])
                                .show(ui, |ui| {
                                    for (idx, (ch, texture)) in slice.iter().enumerate() {
                                        let (cell_rect, cell_resp) = ui.allocate_exact_size(
                                            egui::vec2(36.0, 36.0),
                                            egui::Sense::click(),
                                        );
                                        if cell_resp.hovered() {
                                            ui.painter().rect_filled(
                                                cell_rect,
                                                6.0,
                                                ui.visuals().widgets.hovered.bg_fill,
                                            );
                                        }

                                        let img_rect = cell_rect.shrink(1.0);
                                        ui.painter().image(
                                            texture.id(),
                                            img_rect,
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );

                                        if cell_resp.on_hover_text(ch.as_str()).clicked() {
                                            insert_emoji_at_cursor(
                                                &mut self.input,
                                                &mut self.input_cursor_char,
                                                ch,
                                            );
                                            self.show_emoji_picker = false;
                                        }
                                        if (idx + 1) % 8 == 0 {
                                            ui.end_row();
                                        }
                                    }
                                });
                        });
                })
            {
                picker_rect = Some(window_resp.response.rect);
            }

            if self.show_emoji_picker
                && !emoji_button_clicked
                && ctx.input(|i| i.pointer.any_pressed())
            {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if let Some(rect) = picker_rect {
                        if !rect.contains(pos) {
                            self.show_emoji_picker = false;
                        }
                    }
                }
            }
        }


        // ── Modal de création de groupe ──────────────────────────────────
        if self.show_group_modal {
            let peers = self.state.lock().unwrap().peers.clone();
            let all_peers: Vec<String> = peers.iter().map(|p| p.username.clone()).collect();
            
            let mut is_open = true;
            egui::Window::new("Créer un groupe")
                .fixed_size([400.0, 350.0])
                .resizable(true)
                .collapsible(false)
                .open(&mut is_open)
                .show(ctx, |ui| {
                    ui.label("Nom du groupe:");
                    ui.text_edit_singleline(&mut self.group_name_input);
                    ui.add_space(12.0);

                    ui.label("Sélectionner les pairs à inviter:");
                    ui.add_space(8.0);

                    // ScrollArea pour la liste des pairs avec checkboxes
                    egui::ScrollArea::vertical()
                        .max_height(150.0)
                        .show(ui, |ui| {
                            if all_peers.is_empty() {
                                ui.label("(Aucun pair disponible)");
                            } else {
                                for peer in &all_peers {
                                    let mut is_selected = self.group_members_selected.contains(peer);
                                    let response = ui.checkbox(&mut is_selected, peer);
                                    if response.changed() {
                                        if is_selected {
                                            self.group_members_selected.insert(peer.clone());
                                        } else {
                                            self.group_members_selected.remove(peer);
                                        }
                                    }
                                }
                            }
                        });

                    ui.add_space(12.0);

                    ui.horizontal(|ui| {
                        let trimmed = self.group_name_input.trim();
                        let is_valid_name = !trimmed.is_empty() && trimmed.len() <= 50 && 
                            trimmed.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
                        
                        if is_valid_name {
                            ui.label(egui::RichText::new(format!("✓ {}", trimmed.len())).small().color(egui::Color32::GREEN));
                        } else if !trimmed.is_empty() {
                            ui.label(egui::RichText::new("✗ Nom invalide").small().color(egui::Color32::RED));
                        }
                        
                        if ui.add_enabled(is_valid_name, egui::Button::new("✓ Créer")).clicked() {
                            let group_name = trimmed.to_string();
                            let members: Vec<String> = self.group_members_selected.iter().cloned().collect();
                            
                            if let Some(group) = self.state.lock().unwrap().create_group(group_name, members) {
                                // Broadcaster le groupe à tous les pairs en ligne
                                let create_event = crate::message::GroupEvent {
                                    action: crate::message::GroupAction::Create { group: group.clone() },
                                };
                                
                                let online_peers = self.state.lock().unwrap().get_online_peers();
                                for addr in online_peers {
                                    let req = SendGroupRequest { to_addr: addr, event: create_event.clone() };
                                    let _ = self.send_group_tx.try_send(req);
                                }
                                
                                self.show_group_modal = false;
                                self.group_name_input.clear();
                                self.group_members_selected.clear();
                            }
                        }

                        if ui.button("✕ Annuler").clicked() {
                            self.show_group_modal = false;
                            self.group_name_input.clear();
                            self.group_members_selected.clear();
                        }
                    });
                });

            // Si la croix (×) a été cliquée
            if !is_open {
                self.show_group_modal = false;
            }
        }


        // ── Zone centrale : messages OU vue réseaux ──────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.active_view == AppView::Networks {
                self.show_networks_view(ui);
                return;
            }

            let (selected_conv, my_name, conv_messages, transfer_items) = {
                let s = self.state.lock().unwrap();
                let selected = s.selected_conversation.clone();
                let my_username = s.my_username.clone();
                let msgs = s.get_conversation_messages();
                let conv_msgs: Vec<ChatMessage> = msgs.into_iter().cloned().collect();
                let transfers = s
                    .transfer_state
                    .items
                    .iter()
                    .filter(|transfer| {
                        !transfer.is_finished()
                            || selected
                                .as_ref()
                                .map(|conversation| transfer.conversation.key() == *conversation)
                                .unwrap_or(false)
                    })
                    .take(6)
                    .cloned()
                    .collect::<Vec<_>>();
                (selected, my_username, conv_msgs, transfers)
            };

            let conversation_title = selected_conv.as_deref().unwrap_or("Tous");
            let is_broadcast = selected_conv.is_none();

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.heading(conversation_title);
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button("▾ Actions", |ui| {
                        // Activer/désactiver notifications sonores (global)
                        let sound_text = if self.enable_sound_notifications {
                            "🔊 Désactiver tous les sons"
                        } else {
                            "🔇 Activer tous les sons"
                        };
                        if ui.button(sound_text).clicked() {
                            self.enable_sound_notifications = !self.enable_sound_notifications;
                            ui.close_menu();
                        }

                        // Muet pour ce salon uniquement
                        let this_conv = selected_conv.clone();
                        let is_muted = self.muted_conversations.contains(&this_conv);
                        let mute_text = if is_muted {
                            "🔔 Réactiver les sons de ce salon"
                        } else {
                            "🔕 Muet pour ce salon"
                        };
                        if ui.button(mute_text).clicked() {
                            if is_muted {
                                self.muted_conversations.remove(&this_conv);
                            } else {
                                self.muted_conversations.insert(this_conv);
                            }
                            ui.close_menu();
                        }

                        // Voir les participants
                        if ui.button("👥 Voir les participants").clicked() {
                            self.show_participants = true;
                            ui.close_menu();
                        }

                        // Effacer l'historique : pas disponible sur "Tous"
                        if !is_broadcast {
                            if ui.button("🗑 Effacer l'historique").clicked() {
                                self.state.lock().unwrap().clear_conversation_history();
                                ui.close_menu();
                            }
                        }

                        // Quitter le salon : uniquement pour les groupes (pas privé, pas broadcast)
                        // TODO: différencier groupe vs privé quand les groupes seront implémentés
                    });
                });
            });
            ui.separator();

            // ── Popup participants ─────────────────────────────────────
            if self.show_participants {
                let (conv_name, my_name, selected_conv, peers) = {
                    let s = self.state.lock().unwrap();
                    (
                        s.selected_conversation.clone().unwrap_or_else(|| "Tous".to_string()),
                        s.my_username.clone(),
                        s.selected_conversation.clone(),
                        s.peers.clone(),
                    )
                };
                let is_broadcast = selected_conv.is_none();
                let mut open = self.show_participants;
                egui::Window::new("Participants")
                    .open(&mut open)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new(format!("Conversation : {}", conv_name)).strong());
                        ui.separator();
                        if is_broadcast {
                            // Afficher tous les peers connectés
                            for peer in &peers {
                                ui.horizontal(|ui| {
                                    ui.label("👤");
                                    ui.label(&peer.username);
                                });
                            }
                            if peers.is_empty() {
                                ui.label("Aucun participant connecté");
                            }
                        } else {
                            // Conversation privée : moi + l'autre
                            ui.horizontal(|ui| {
                                ui.label("👤");
                                ui.label(format!("{} (vous)", my_name));
                            });
                            if let Some(peer) = selected_conv {
                                ui.horizontal(|ui| {
                                    ui.label("👤");
                                    ui.label(&peer);
                                });
                            }
                        }
                    });
                self.show_participants = open;
            }

            if !transfer_items.is_empty() {
                ui.label(egui::RichText::new("Transferts").strong());
                ui.add_space(4.0);
                for transfer in &transfer_items {
                    render_transfer_card(ui, transfer);
                    ui.add_space(4.0);
                }
                ui.separator();
            }

            // ── Messages filtrés ────────────────────────────────────────
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if conv_messages.is_empty() {
                        ui.add_space(50.0);
                        ui.label(egui::RichText::new("Aucun message").weak());
                    }
                    
                    for msg in &conv_messages {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("[{}]", msg.timestamp))
                                        .color(egui::Color32::DARK_GRAY)
                                        .small(),
                                );
                                let name_color = if msg.from == my_name {
                                    egui::Color32::from_rgb(80, 200, 120)
                                } else {
                                    egui::Color32::from_rgb(100, 180, 255)
                                };
                                ui.label(
                                    egui::RichText::new(format!("{}:", msg.from))
                                        .color(name_color)
                                        .strong(),
                                );
                            });
                            // Message content - render inline with horizontal_wrapped for automatic wrapping
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                render_inline(
                                    ui,
                                    &msg.content,
                                    &self.emoji_map,
                                    &self.emoji_textures,
                                    16.0,
                                );
                            });
                        });
                    }
                });
        });
    }
}

impl AbcomApp {
    fn show_networks_view(&mut self, ui: &mut egui::Ui) {
        let (known_networks, peer_records, current_network_id) = {
            let s = self.state.lock().unwrap();
            (s.known_networks.clone(), s.peer_records.clone(), s.current_network_id.clone())
        };

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading("🌐 Réseaux connus");
        });
        ui.separator();

        if known_networks.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Aucun réseau enregistré").weak());
                ui.label(egui::RichText::new("Les réseaux apparaissent automatiquement quand vous détectez des pairs.").weak().small());
            });
            return;
        }

        // Panel gauche : liste des réseaux sous forme de cards
        egui::SidePanel::left("networks_list_panel")
            .resizable(false)
            .exact_width(220.0)
            .show_inside(ui, |ui| {
                ui.add_space(4.0);
                for net in &known_networks {
                    let is_selected = self.networks_view_selected.as_ref() == Some(&net.id);
                    let is_current = current_network_id.as_ref() == Some(&net.id);

                    let desired = egui::vec2(ui.available_width(), 72.0);
                    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
                    let fill = if is_selected {
                        ui.visuals().selection.bg_fill
                    } else if resp.hovered() {
                        ui.visuals().widgets.hovered.bg_fill
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    ui.painter().rect_filled(rect, 8.0, fill);

                    let icon = if is_current { "📡" } else { "🔌" };
                    let title = net.display_name();
                    let subtitle = if !net.subnet.is_empty() {
                        format!("{} pair(s) • {}.x", net.seen_peers.len(), net.subnet)
                    } else {
                        format!("{} pair(s)", net.seen_peers.len())
                    };
                    let text_color = ui.visuals().text_color();
                    let font = egui::TextStyle::Button.resolve(ui.style());
                    let small_font = egui::TextStyle::Small.resolve(ui.style());
                    ui.painter().text(rect.left_top() + egui::vec2(10.0, 16.0), egui::Align2::LEFT_TOP,
                        format!("{} {}", icon, title), font, text_color);
                    ui.painter().text(rect.left_top() + egui::vec2(10.0, 38.0), egui::Align2::LEFT_TOP,
                        &subtitle, small_font, egui::Color32::GRAY);
                    if is_current {
                        let badge_font = egui::TextStyle::Small.resolve(ui.style());
                        ui.painter().text(rect.right_top() + egui::vec2(-8.0, 14.0), egui::Align2::RIGHT_TOP,
                            "actuel", badge_font, egui::Color32::from_rgb(50, 200, 80));
                    }
                    if resp.clicked() {
                        self.networks_view_selected = Some(net.id.clone());
                        self.editing_network_alias = None;
                    }
                    ui.add_space(4.0);
                }
            });

        // Zone droite : détail du réseau sélectionné
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let Some(ref network_id) = self.networks_view_selected.clone() else {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("← Sélectionnez un réseau").weak());
                });
                return;
            };
            let Some(net) = known_networks.iter().find(|n| &n.id == network_id).cloned() else {
                return;
            };

            ui.add_space(8.0);
            // Titre + édition alias
            ui.horizontal(|ui| {
                ui.heading(format!("📡 {}", net.display_name()));
                if !net.subnet.is_empty() {
                    ui.label(egui::RichText::new(format!("  {}.x", net.subnet)).weak().small());
                }
                if let Some((ref edit_id, ref mut buf)) = self.editing_network_alias {
                    if edit_id == network_id {
                        let resp = ui.text_edit_singleline(buf);
                        if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            let new_alias = buf.trim().to_string();
                            let mut s = self.state.lock().unwrap();
                            if let Some(n) = s.known_networks.iter_mut().find(|n| &n.id == network_id) {
                                n.alias = if new_alias.is_empty() { None } else { Some(new_alias) };
                            }
                            s.save_networks();
                            self.editing_network_alias = None;
                        }
                    }
                } else {
                    if ui.small_button("✏ Renommer").clicked() {
                        let current = net.alias.clone().unwrap_or_default();
                        self.editing_network_alias = Some((network_id.clone(), current));
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(egui::RichText::new("🗑 Oublier ce réseau").color(egui::Color32::from_rgb(220, 60, 60))).clicked() {
                        self.state.lock().unwrap().forget_network(network_id);
                        self.networks_view_selected = None;
                    }
                });
            });
            ui.separator();
            ui.add_space(8.0);

            // Cards des pairs de ce réseau
            ui.label(egui::RichText::new(format!("{} paire(s) connu(s) sur ce réseau", net.seen_peers.len())).small().weak());
            ui.add_space(8.0);

            if net.seen_peers.is_empty() {
                ui.weak("Aucun pair sur ce réseau.");
                return;
            }

            let peers_state = {
                let s = self.state.lock().unwrap();
                s.peers.clone()
            };

            egui::ScrollArea::vertical().show(ui, |ui| {
                let card_w = 180.0;
                let card_h = 90.0;
                let columns = ((ui.available_width() + 12.0) / (card_w + 12.0)).floor().max(1.0) as usize;
                let mut col = 0usize;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                    for username in &net.seen_peers {
                        let peer_live = peers_state.iter().find(|p| &p.username == username);
                        let online = peer_live.map(|p| p.online).unwrap_or(false);
                        let alias = peer_records.iter()
                            .find(|r| &r.username == username)
                            .and_then(|r| r.alias.clone());
                        let display = alias.clone().unwrap_or_else(|| username.clone());

                        let (card_rect, card_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                        let card_fill = if card_resp.hovered() {
                            ui.visuals().widgets.hovered.bg_fill
                        } else {
                            ui.visuals().widgets.inactive.bg_fill
                        };
                        ui.painter().rect_filled(card_rect, 10.0, card_fill);
                        ui.painter().rect_stroke(card_rect, 10.0, ui.visuals().widgets.inactive.bg_stroke, egui::StrokeKind::Outside);

                        // Dot online
                        let dot_color = if online { egui::Color32::from_rgb(50, 200, 80) } else { egui::Color32::GRAY };
                        ui.painter().circle_filled(card_rect.left_top() + egui::vec2(14.0, 14.0), 5.0, dot_color);

                        // Nom affiché
                        let name_font = egui::TextStyle::Button.resolve(ui.style());
                        ui.painter().text(card_rect.center_top() + egui::vec2(0.0, 14.0), egui::Align2::CENTER_TOP,
                            &display, name_font, ui.visuals().text_color());

                        // Username en petit si alias
                        if alias.is_some() {
                            let small = egui::TextStyle::Small.resolve(ui.style());
                            ui.painter().text(card_rect.center_top() + egui::vec2(0.0, 35.0), egui::Align2::CENTER_TOP,
                                username, small, egui::Color32::GRAY);
                        }

                        // Édition alias pair
                        let is_editing = self.editing_peer_alias.as_ref().map(|(u, _)| u == username).unwrap_or(false);
                        let btn_rect = egui::Rect::from_min_size(
                            card_rect.left_bottom() + egui::vec2(8.0, -30.0),
                            egui::vec2(card_w - 16.0, 22.0),
                        );

                        if is_editing {
                            if let Some((_, ref mut buf)) = self.editing_peer_alias {
                                let resp = ui.put(btn_rect, egui::TextEdit::singleline(buf).hint_text("Alias..."));
                                if resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    let new_alias = buf.trim().to_string();
                                    let mut s = self.state.lock().unwrap();
                                    if let Some(rec) = s.peer_records.iter_mut().find(|r| &r.username == username) {
                                        rec.alias = if new_alias.is_empty() { None } else { Some(new_alias) };
                                    } else {
                                        s.peer_records.push(crate::message::PeerRecord {
                                            username: username.clone(),
                                            alias: if new_alias.is_empty() { None } else { Some(new_alias) },
                                            last_subnet: Some(network_id.clone()),
                                        });
                                    }
                                    s.save_peer_records();
                                    self.editing_peer_alias = None;
                                }
                            }
                        } else {
                            let edit_btn = ui.put(btn_rect, egui::Button::new(
                                egui::RichText::new("✏ Alias").small()
                            ).frame(false));
                            if edit_btn.clicked() {
                                self.editing_peer_alias = Some((username.clone(), alias.clone().unwrap_or_default()));
                            }
                        }

                        col += 1;
                        if col >= columns { col = 0; }
                    }
                });
            });
        });
    }
}
