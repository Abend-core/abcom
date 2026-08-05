pub mod text_ops;

pub use text_ops::{insert_emoji_at_cursor, replace_char_range};

use eframe::egui;

use self::text_ops::{
    char_prefix, char_range_string, insert_text_at_cursor, line_end, line_start, next_word_end,
    prev_word_start,
};

pub fn sync_cursor(_ctx: &egui::Context, _char_pos: usize) {}

/// Plafond de saisie du composeur, en caractères Unicode (pas en octets :
/// accents et emoji comptent pour un). Protège le coût de layout par frappe,
/// pas le protocole — la limite réseau (8 Mio) est vérifiée à l'envoi.
pub const MAX_INPUT_CHARS: usize = 100_000;

/// Normalise les fins de ligne d'un texte collé (`\r\n` et `\r` → `\n`),
/// en conservant les retours à la ligne — le fil les affiche tels quels.
fn normalize_paste(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnterKeyAction {
    InsertNewline,
    AcceptShortcode,
    Submit,
}

/// Entrée insère une nouvelle ligne (comme Shift+Entrée) ; l'envoi se fait par
/// Cmd+Entrée (macOS) ou Ctrl+Entrée. Entrée seule valide le shortcode quand le
/// menu de suggestions est ouvert.
fn enter_key_action(shortcode_menu_open: bool, modifiers: egui::Modifiers) -> EnterKeyAction {
    if modifiers.command || modifiers.ctrl {
        EnterKeyAction::Submit
    } else if shortcode_menu_open && !modifiers.shift {
        EnterKeyAction::AcceptShortcode
    } else {
        EnterKeyAction::InsertNewline
    }
}

fn accept_selected_shortcode(
    input: &mut String,
    cursor_char: &mut usize,
    emoji_alias_to_char: &std::collections::HashMap<String, String>,
    emoji_aliases: &[String],
    shortcode_selected: usize,
) -> bool {
    let Some((start, _query)) =
        crate::ui::emoji_picker::emoji_shortcode_trigger(input, *cursor_char)
    else {
        return false;
    };
    let suggestions = crate::ui::emoji_picker::shortcode_suggestions(
        input,
        *cursor_char,
        emoji_alias_to_char,
        emoji_aliases,
        shortcode_selected.saturating_add(1),
    );
    let Some((_alias, ch)) =
        suggestions.get(shortcode_selected.min(suggestions.len().saturating_sub(1)))
    else {
        return false;
    };

    replace_char_range(input, cursor_char, start, *cursor_char, ch);
    true
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
    max_width: f32,
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
        for len in [2usize, 1usize] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if emoji_map.contains_key(&s) {
                    let advance = emoji_size + 2.0;
                    if x + advance > max_width && x > 0.0 {
                        x = 0.0;
                        y += line_height;
                    }
                    x += advance;
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
            let advance = measure_text_width(ui, &ch);
            if x + advance > max_width && x > 0.0 {
                x = 0.0;
                y += line_height;
            }
            x += advance;
            i += 1;
            points.push(egui::pos2(x, y));
        }
    }

    points
}

fn visual_line_count(caret_points: &[egui::Pos2], line_height: f32) -> usize {
    caret_points
        .last()
        .map(|p| (p.y / line_height).floor() as usize + 1)
        .unwrap_or(1)
        .max(1)
}

/// Position de curseur la plus proche d'un point cliqué (en coordonnées de
/// contenu, défilement déjà compensé par l'appelant).
///
/// On choisit **d'abord la bonne ligne** (par la position verticale), puis, sur
/// cette ligne, la colonne la plus proche. Un plus-proche 2D naïf sauterait sur
/// la ligne du dessus quand on clique à droite d'une ligne courte : la distance
/// horizontale (ligne longue au-dessus) l'emporterait sur l'espacement vertical.
fn cursor_from_point(points: &[egui::Pos2], target: egui::Pos2, line_height: f32) -> usize {
    if points.len() <= 1 {
        return 0;
    }
    let max_line = points
        .iter()
        .map(|p| (p.y / line_height).round() as i32)
        .max()
        .unwrap_or(0);
    let target_line = ((target.y / line_height).round() as i32).clamp(0, max_line);

    let mut best_idx = None;
    let mut best_dx = f32::MAX;
    for (idx, p) in points.iter().enumerate() {
        if (p.y / line_height).round() as i32 == target_line {
            let dx = (p.x - target.x).abs();
            if dx < best_dx {
                best_dx = dx;
                best_idx = Some(idx);
            }
        }
    }
    // La ligne visée contient toujours au moins une position ; repli défensif
    // sur la fin du texte si ce n'était pas le cas.
    best_idx.unwrap_or(points.len() - 1)
}

fn selection_range(selection_anchor: Option<usize>, cursor_char: usize) -> Option<(usize, usize)> {
    selection_anchor.and_then(|anchor| {
        if anchor == cursor_char {
            None
        } else {
            Some((anchor.min(cursor_char), anchor.max(cursor_char)))
        }
    })
}

fn clear_selection(selection_anchor: &mut Option<usize>) {
    *selection_anchor = None;
}

fn replace_selection(
    text: &mut String,
    cursor: &mut usize,
    selection_anchor: &mut Option<usize>,
    replacement: &str,
) -> bool {
    if let Some((start, end)) = selection_range(*selection_anchor, *cursor) {
        replace_char_range(text, cursor, start, end, replacement);
        *selection_anchor = None;
        true
    } else {
        false
    }
}

fn paint_selection(
    painter: &egui::Painter,
    content_rect: egui::Rect,
    caret_points: &[egui::Pos2],
    selection: Option<(usize, usize)>,
    scroll_lines: f32,
    line_height: f32,
    color: egui::Color32,
) {
    let (start, end) = match selection {
        Some(range) => range,
        None => return,
    };
    // `end` indexe `caret_points` directement : il doit rester strictement
    // sous `len` (le dernier point valide est `len - 1`).
    if start >= end || end >= caret_points.len() {
        return;
    }

    let mut line_start = start;
    let mut line_y = caret_points[start].y;
    for idx in (start + 1)..=end {
        let current_y = if idx < caret_points.len() {
            caret_points[idx].y
        } else {
            caret_points[end - 1].y
        };

        if idx == end || (current_y - line_y).abs() > f32::EPSILON {
            let start_x = caret_points[line_start].x;
            let end_x = if idx == end {
                caret_points[end].x
            } else {
                content_rect.width()
            };
            let top = content_rect.top() + line_y + 2.0 - scroll_lines * line_height;
            let bottom = top + 18.0;
            let rect = egui::Rect::from_min_max(
                egui::pos2(content_rect.left() + start_x, top),
                egui::pos2(content_rect.left() + end_x, bottom),
            )
            .intersect(content_rect);
            if rect.is_positive() {
                painter.rect_filled(rect, 0.0, color);
            }
            line_start = idx;
            if idx < caret_points.len() {
                line_y = caret_points[idx].y;
            }
        }
    }
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

    let mut best_idx = None;
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

// Widget de saisie de bas niveau : état du texte/curseur/sélection et contexte
// de rendu passés séparément ; un struct n'améliorerait pas la lisibilité.
// Retourne (réponse, envoi demandé, texte modifié, collage débordant le
// plafond — à transformer en pièce jointe par l'appelant).
#[allow(clippy::too_many_arguments)]
pub fn custom_composer_input(
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
    shortcode_selected: usize,
    width: f32,
    selection_anchor: &mut Option<usize>,
) -> (egui::Response, bool, bool, Option<String>) {
    let line_height = 22.0;
    let base_content_width = (width.max(120.0) - 12.0).max(20.0);
    let initial_caret_points =
        composer_caret_positions(ui, input, emoji_map, 18.0, base_content_width);
    let mut line_count = visual_line_count(&initial_caret_points, line_height);
    let needs_scrollbar = line_count > 10;
    // La largeur de contenu de chaque branche coïncide avec celle du
    // `content_rect` correspondant : les points calculés ici sont réutilisés
    // tels quels pour le rendu (une seule passe de mesure par frame).
    let mut caret_points = if needs_scrollbar {
        let content_width = (width.max(120.0) - 20.0).max(20.0);
        let points = composer_caret_positions(ui, input, emoji_map, 18.0, content_width);
        line_count = visual_line_count(&points, line_height);
        points
    } else {
        initial_caret_points
    };
    let visual_lines = line_count.clamp(1, 10) as f32;
    let desired_size = egui::vec2(width.max(120.0), 10.0 + visual_lines * line_height);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
    let content_rect = if needs_scrollbar {
        egui::Rect::from_min_max(
            rect.min + egui::vec2(6.0, 5.0),
            rect.max - egui::vec2(14.0, 5.0),
        )
    } else {
        rect.shrink2(egui::vec2(6.0, 5.0))
    };
    let max_scroll = (line_count as f32 - visual_lines).max(0.0);
    *scroll_lines = scroll_lines.clamp(0.0, max_scroll);

    // Position écran → coordonnées de contenu pour placer le curseur : compense
    // le défilement vertical et l'offset de centrage des lignes (le texte est
    // peint centré à +11 px, cf. rendu), pour qu'un clic tombe sur la ligne
    // réellement sous le pointeur, y compris quand l'input est défilé.
    let scroll_px = *scroll_lines * line_height;
    let to_content = |pos: egui::Pos2| {
        egui::pos2(
            (pos.x - content_rect.left()).max(0.0),
            pos.y - content_rect.top() + scroll_px - 11.0,
        )
    };

    if ui.input(|i| i.pointer.any_pressed()) && response.hovered() {
        if !ui.input(|i| i.modifiers.shift) {
            clear_selection(selection_anchor);
        }
        if let Some(pos) = response.interact_pointer_pos() {
            let pressed_cursor = cursor_from_point(&caret_points, to_content(pos), line_height);
            if selection_anchor.is_none() {
                *selection_anchor = Some(pressed_cursor);
            }
            *cursor_char = pressed_cursor;
        }
    }

    if response.clicked() {
        *input_has_focus = true;
        response.request_focus();
        let clicked_cursor = if let Some(pos) = response.interact_pointer_pos() {
            cursor_from_point(&caret_points, to_content(pos), line_height)
        } else {
            input.chars().count()
        };
        if ui.input(|i| i.modifiers.shift) {
            if selection_anchor.is_none() {
                *selection_anchor = Some(*cursor_char);
            }
            *cursor_char = clicked_cursor;
        } else {
            if selection_range(*selection_anchor, clicked_cursor).is_none() {
                clear_selection(selection_anchor);
            }
            *cursor_char = clicked_cursor;
        }
    }

    if response.drag_started() && selection_anchor.is_none() {
        if let Some(pos) = response.interact_pointer_pos() {
            let drag_start_cursor = cursor_from_point(&caret_points, to_content(pos), line_height);
            *selection_anchor = Some(drag_start_cursor);
            *cursor_char = drag_start_cursor;
        }
    }

    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            *cursor_char = cursor_from_point(&caret_points, to_content(pos), line_height);
        }
    }

    let has_focus = *input_has_focus || response.has_focus();
    let mut changed = false;
    let mut submit = false;
    // Collage dépassant le plafond : renvoyé intact à l'appelant (qui le
    // transforme en pièce jointe .txt) au lieu d'être tronqué ou inséré.
    let mut overflow_paste: Option<String> = None;
    let total_chars = input.chars().count();
    if *cursor_char > total_chars {
        *cursor_char = total_chars;
    }

    // Position du curseur avant traitement clavier : si une frappe ou une
    // navigation la déplace, on fait défiler l'input pour la garder visible
    // (plus bas). On ne resnappe PAS à chaque frame — sinon la molette et
    // l'ascenseur ne pourraient jamais défiler loin du curseur.
    let cursor_before = *cursor_char;

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
                    egui::pos2(
                        cursor_x + 1.0,
                        cursor_bottom.max(cursor_top.max(content_rect.top())),
                    ),
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
                egui::Event::Text(t) if !t.contains('\n') && !t.contains('\r') => {
                    replace_selection(input, cursor_char, selection_anchor, "");
                    let room = MAX_INPUT_CHARS.saturating_sub(input.chars().count());
                    let to_insert = char_prefix(&t, room);
                    if !to_insert.is_empty() {
                        insert_text_at_cursor(input, cursor_char, to_insert);
                    }
                    changed = true;
                }
                egui::Event::Ime(egui::ImeEvent::Commit(t))
                    if !t.contains('\n') && !t.contains('\r') && !t.is_empty() =>
                {
                    replace_selection(input, cursor_char, selection_anchor, "");
                    let room = MAX_INPUT_CHARS.saturating_sub(input.chars().count());
                    let to_insert = char_prefix(&t, room);
                    if !to_insert.is_empty() {
                        insert_text_at_cursor(input, cursor_char, to_insert);
                    }
                    changed = true;
                }
                egui::Event::Paste(t) => {
                    // Retours à la ligne conservés (le fil est multiligne).
                    let pasted = normalize_paste(&t);
                    let selected = selection_range(*selection_anchor, *cursor_char)
                        .map(|(start, end)| end - start)
                        .unwrap_or(0);
                    let after = input.chars().count() - selected + pasted.chars().count();
                    if after > MAX_INPUT_CHARS {
                        overflow_paste = Some(pasted);
                    } else {
                        replace_selection(input, cursor_char, selection_anchor, "");
                        insert_text_at_cursor(input, cursor_char, &pasted);
                        changed = true;
                    }
                }
                egui::Event::Copy => {
                    if let Some((start, end)) = selection_range(*selection_anchor, *cursor_char) {
                        ui.ctx().copy_text(char_range_string(input, start, end));
                    }
                }
                egui::Event::Cut => {
                    if let Some((start, end)) = selection_range(*selection_anchor, *cursor_char) {
                        ui.ctx().copy_text(char_range_string(input, start, end));
                        replace_char_range(input, cursor_char, start, end, "");
                        clear_selection(selection_anchor);
                        changed = true;
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => match key {
                    egui::Key::A if modifiers.ctrl || modifiers.command => {
                        let total_chars = input.chars().count();
                        if total_chars > 0 {
                            *selection_anchor = Some(0);
                            *cursor_char = total_chars;
                        } else {
                            clear_selection(selection_anchor);
                        }
                    }
                    egui::Key::Enter => match enter_key_action(shortcode_menu_open, modifiers) {
                        EnterKeyAction::InsertNewline => {
                            replace_selection(input, cursor_char, selection_anchor, "");
                            if input.chars().count() < MAX_INPUT_CHARS {
                                insert_text_at_cursor(input, cursor_char, "\n");
                            }
                            changed = true;
                        }
                        EnterKeyAction::AcceptShortcode => {
                            changed |= accept_selected_shortcode(
                                input,
                                cursor_char,
                                emoji_alias_to_char,
                                emoji_aliases,
                                shortcode_selected,
                            );
                        }
                        EnterKeyAction::Submit => {
                            submit = true;
                        }
                    },
                    egui::Key::Tab => {
                        let suggestions = crate::ui::emoji_picker::shortcode_suggestions(
                            input,
                            *cursor_char,
                            emoji_alias_to_char,
                            emoji_aliases,
                            1,
                        );
                        if let Some((_alias, ch)) = suggestions.first() {
                            if let Some((start, _query)) =
                                crate::ui::emoji_picker::emoji_shortcode_trigger(
                                    input,
                                    *cursor_char,
                                )
                            {
                                replace_char_range(input, cursor_char, start, *cursor_char, ch);
                                changed = true;
                            }
                        }
                    }
                    egui::Key::Backspace => {
                        if replace_selection(input, cursor_char, selection_anchor, "") {
                            changed = true;
                        } else {
                            // Cmd+Backspace : jusqu'au début de ligne (macOS) ;
                            // Option/Ctrl+Backspace : mot précédent ;
                            // sinon caractère précédent.
                            let target = if modifiers.mac_cmd {
                                line_start(input, *cursor_char)
                            } else if modifiers.alt || modifiers.ctrl {
                                prev_word_start(input, *cursor_char)
                            } else {
                                cursor_char.saturating_sub(1)
                            };
                            if target < *cursor_char {
                                replace_char_range(input, cursor_char, target, *cursor_char, "");
                                changed = true;
                            }
                        }
                    }
                    egui::Key::Delete => {
                        if replace_selection(input, cursor_char, selection_anchor, "") {
                            changed = true;
                        } else {
                            // Option/Ctrl+Delete : mot suivant ; sinon caractère
                            // suivant.
                            let target = if modifiers.alt || modifiers.ctrl {
                                next_word_end(input, *cursor_char)
                            } else {
                                (*cursor_char + 1).min(input.chars().count())
                            };
                            if target > *cursor_char {
                                replace_char_range(input, cursor_char, *cursor_char, target, "");
                                changed = true;
                            }
                        }
                    }
                    egui::Key::ArrowLeft => {
                        if modifiers.shift {
                            if selection_anchor.is_none() {
                                *selection_anchor = Some(*cursor_char);
                            }
                        } else {
                            clear_selection(selection_anchor);
                        }
                        // Cmd+← : début de ligne (macOS) ; Option/Ctrl+← : mot
                        // précédent ; sinon caractère précédent.
                        *cursor_char = if modifiers.mac_cmd {
                            line_start(input, *cursor_char)
                        } else if modifiers.alt || modifiers.ctrl {
                            prev_word_start(input, *cursor_char)
                        } else {
                            cursor_char.saturating_sub(1)
                        };
                        if selection_range(*selection_anchor, *cursor_char).is_none() {
                            clear_selection(selection_anchor);
                        }
                    }
                    egui::Key::ArrowRight => {
                        if modifiers.shift {
                            if selection_anchor.is_none() {
                                *selection_anchor = Some(*cursor_char);
                            }
                        } else {
                            clear_selection(selection_anchor);
                        }
                        // Cmd+→ : fin de ligne (macOS) ; Option/Ctrl+→ : mot
                        // suivant ; sinon caractère suivant.
                        *cursor_char = if modifiers.mac_cmd {
                            line_end(input, *cursor_char)
                        } else if modifiers.alt || modifiers.ctrl {
                            next_word_end(input, *cursor_char)
                        } else {
                            (*cursor_char + 1).min(input.chars().count())
                        };
                        if selection_range(*selection_anchor, *cursor_char).is_none() {
                            clear_selection(selection_anchor);
                        }
                    }
                    egui::Key::ArrowUp if !shortcode_menu_open => {
                        if modifiers.shift && selection_anchor.is_none() {
                            *selection_anchor = Some(*cursor_char);
                        } else if !modifiers.shift {
                            clear_selection(selection_anchor);
                        }
                        let points = composer_caret_positions(
                            ui,
                            input,
                            emoji_map,
                            18.0,
                            content_rect.width().max(20.0),
                        );
                        move_cursor_vertical(&points, cursor_char, -1, line_height);
                        if selection_range(*selection_anchor, *cursor_char).is_none() {
                            clear_selection(selection_anchor);
                        }
                    }
                    egui::Key::ArrowDown if !shortcode_menu_open => {
                        if modifiers.shift && selection_anchor.is_none() {
                            *selection_anchor = Some(*cursor_char);
                        } else if !modifiers.shift {
                            clear_selection(selection_anchor);
                        }
                        let points = composer_caret_positions(
                            ui,
                            input,
                            emoji_map,
                            18.0,
                            content_rect.width().max(20.0),
                        );
                        move_cursor_vertical(&points, cursor_char, 1, line_height);
                        if selection_range(*selection_anchor, *cursor_char).is_none() {
                            clear_selection(selection_anchor);
                        }
                    }
                    egui::Key::Home => {
                        if modifiers.shift && selection_anchor.is_none() {
                            *selection_anchor = Some(*cursor_char);
                        } else if !modifiers.shift {
                            clear_selection(selection_anchor);
                        }
                        *cursor_char = 0;
                        if selection_range(*selection_anchor, *cursor_char).is_none() {
                            clear_selection(selection_anchor);
                        }
                    }
                    egui::Key::End => {
                        if modifiers.shift && selection_anchor.is_none() {
                            *selection_anchor = Some(*cursor_char);
                        } else if !modifiers.shift {
                            clear_selection(selection_anchor);
                        }
                        *cursor_char = input.chars().count();
                        if selection_range(*selection_anchor, *cursor_char).is_none() {
                            clear_selection(selection_anchor);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    // Les événements ci-dessus ont pu modifier le texte : re-clampe curseur et
    // ancre de sélection puis recalcule les positions de caractères avant le
    // rendu, sinon la peinture de la sélection lit des points périmés (panique
    // « index out of bounds » quand saisie et sélection tombent dans la même
    // frame).
    let total_chars = input.chars().count();
    if *cursor_char > total_chars {
        *cursor_char = total_chars;
    }
    if let Some(anchor) = *selection_anchor {
        if anchor > total_chars {
            *selection_anchor = Some(total_chars);
        }
    }
    if changed {
        caret_points =
            composer_caret_positions(ui, input, emoji_map, 18.0, content_rect.width().max(20.0));
    }

    // Défilement suiveur : après une frappe ou une navigation clavier, garder la
    // ligne du curseur dans la fenêtre visible (flèches haut/bas, saisie qui
    // pousse le texte hors champ). N'agit que si le curseur a bougé, pour ne pas
    // annuler un défilement molette/ascenseur.
    if *cursor_char != cursor_before {
        let recomputed_max_scroll =
            (visual_line_count(&caret_points, line_height) as f32 - visual_lines).max(0.0);
        let caret_line = caret_points
            .get(*cursor_char)
            .map(|p| (p.y / line_height).round())
            .unwrap_or(0.0);
        if caret_line < *scroll_lines {
            *scroll_lines = caret_line;
        } else if caret_line > *scroll_lines + visual_lines - 1.0 {
            *scroll_lines = caret_line - visual_lines + 1.0;
        }
        *scroll_lines = scroll_lines.clamp(0.0, recomputed_max_scroll);
    }

    let frame_fill = egui::Color32::TRANSPARENT;
    let frame_stroke = egui::Stroke::NONE;

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(12),
        frame_fill,
        frame_stroke,
        egui::StrokeKind::Outside,
    );

    if input.is_empty() {
        ui.painter().text(
            content_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            "Send a message... Ctrl/Cmd + Enter ",
            egui::TextStyle::Body.resolve(ui.style()),
            egui::Color32::from_rgb(185, 187, 192),
        );
    } else {
        let painter = ui.painter().with_clip_rect(content_rect);
        paint_selection(
            &painter,
            content_rect,
            &caret_points,
            selection_range(*selection_anchor, *cursor_char),
            *scroll_lines,
            line_height,
            ui.visuals().selection.bg_fill,
        );

        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;
        let mut x = content_rect.left();
        let right = content_rect.right();
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
            for len in [2usize, 1usize] {
                if i + len <= chars.len() {
                    let s: String = chars[i..i + len].iter().collect();
                    if let Some(&idx) = emoji_map.get(&s) {
                        if let Some((_, tex)) = emoji_textures.get(idx) {
                            if x + 20.0 > right && x > content_rect.left() {
                                x = content_rect.left();
                                y += line_height;
                            }
                            let img_rect = egui::Rect::from_min_size(
                                egui::pos2(x, y - 9.0),
                                egui::vec2(18.0, 18.0),
                            );
                            painter.image(
                                tex.id(),
                                img_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
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
                let glyph = chars[i].to_string();
                let glyph_w = measure_text_width(ui, &glyph);
                if x + glyph_w > right && x > content_rect.left() {
                    x = content_rect.left();
                    y += line_height;
                }
                painter.text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    &glyph,
                    egui::TextStyle::Body.resolve(ui.style()),
                    egui::Color32::from_rgb(244, 245, 247),
                );
                x += glyph_w;
                i += 1;
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
            let t = if max_scroll <= 0.0 {
                0.0
            } else {
                *scroll_lines / max_scroll
            };
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
        // Le clignotement a besoin d'une frame à chaque bascule (250 ms) ;
        // sans ça le trait reste figé jusqu'au prochain repaint (jusqu'à 5 s
        // au repos). Uniquement fenêtre au premier plan : en arrière-plan on
        // garde le rythme quasi dormant.
        if ui.input(|i| i.focused) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
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
                    [
                        egui::pos2(x, top.max(content_rect.top())),
                        egui::pos2(x, bottom),
                    ],
                    egui::Stroke::new(1.6, egui::Color32::from_rgb(250, 250, 252)),
                );
            }
        }
    }

    (response, submit, changed, overflow_paste)
}

#[cfg(test)]
#[path = "../../tests/test_ui_composer_mod.rs"]
mod tests;
