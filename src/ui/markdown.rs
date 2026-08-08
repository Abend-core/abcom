use eframe::egui;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MarkdownBlock {
    Paragraph(Vec<MarkdownSpan>),
    Heading {
        level: usize,
        spans: Vec<MarkdownSpan>,
    },
    Bullet(Vec<MarkdownSpan>),
    OrderedBullet {
        number: usize,
        spans: Vec<MarkdownSpan>,
    },
    TaskItem {
        checked: bool,
        spans: Vec<MarkdownSpan>,
    },
    Blockquote(Vec<MarkdownSpan>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Table {
        alignments: Vec<ColumnAlignment>,
        header: Vec<Vec<MarkdownSpan>>,
        rows: Vec<Vec<Vec<MarkdownSpan>>>,
    },
    ThematicBreak,
    Blank,
}

/// Alignement d'une colonne de tableau (dérivé de la ligne de séparation
/// GitHub-flavored : `:---`, `:---:`, `---:`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColumnAlignment {
    None,
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MarkdownSpan {
    Text(String),
    Strong(String),
    Emphasis(String),
    Strikethrough(String),
    Code(String),
    Link { label: String, url: String },
}

pub(crate) fn parse_markdown(input: &str) -> Vec<MarkdownBlock> {
    if input.is_empty() {
        return vec![MarkdownBlock::Paragraph(Vec::new())];
    }

    let mut blocks = Vec::new();
    let mut lines = input.lines().peekable();

    while let Some(line) = lines.next() {
        if let Some(language) = fenced_code_language(line) {
            push_pending_blank(&mut blocks);
            let mut code = String::new();

            for code_line in lines.by_ref() {
                if is_code_fence(code_line) {
                    break;
                }
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(code_line);
            }
            trim_trailing_blank_lines(&mut code);

            blocks.push(MarkdownBlock::CodeBlock { language, code });
            continue;
        }

        let trimmed = line.trim();

        if trimmed.is_empty() {
            blocks.push(MarkdownBlock::Blank);
            continue;
        }

        // Tableau GitHub-flavored : une ligne d'en-tête suivie d'une ligne de
        // séparation (`| --- | :---: |`). On regarde la ligne suivante avant
        // de s'engager pour ne pas confondre un paragraphe contenant des `|`.
        if line.contains('|') {
            if let Some(delimiter) = lines.peek().copied() {
                if is_table_delimiter(delimiter) {
                    let alignments = parse_table_alignments(delimiter);
                    let header = parse_table_row(line, alignments.len());
                    lines.next(); // consomme la ligne de séparation

                    let mut rows = Vec::new();
                    while let Some(row_line) = lines.peek().copied() {
                        if row_line.trim().is_empty() || !row_line.contains('|') {
                            break;
                        }
                        rows.push(parse_table_row(row_line, alignments.len()));
                        lines.next();
                    }

                    blocks.push(MarkdownBlock::Table {
                        alignments,
                        header,
                        rows,
                    });
                    continue;
                }
            }
        }

        if let Some((level, text)) = heading_text(line) {
            blocks.push(MarkdownBlock::Heading {
                level,
                spans: parse_inline(text),
            });
            continue;
        }

        if let Some((checked, text)) = task_item_text(line) {
            blocks.push(MarkdownBlock::TaskItem {
                checked,
                spans: parse_inline(text),
            });
            continue;
        }

        if let Some(text) = bullet_text(line) {
            blocks.push(MarkdownBlock::Bullet(parse_inline(text)));
            continue;
        }

        if let Some((number, text)) = ordered_bullet_text(line) {
            blocks.push(MarkdownBlock::OrderedBullet {
                number,
                spans: parse_inline(text),
            });
            continue;
        }

        if let Some(text) = blockquote_text(line) {
            blocks.push(MarkdownBlock::Blockquote(parse_inline(text)));
            continue;
        }

        if is_thematic_break(trimmed) {
            blocks.push(MarkdownBlock::ThematicBreak);
            continue;
        }

        // Sémantique chat (pas CommonMark) : un Entrée dans le composeur est
        // un vrai retour à la ligne, conservé tel quel dans le paragraphe
        // (egui rend les `\n` d'un label comme des sauts de ligne).
        let mut paragraph = trimmed.to_string();
        while let Some(next_line) = lines.peek().copied() {
            let next_trimmed = next_line.trim();
            if next_trimmed.is_empty() || starts_special_block(next_line) {
                break;
            }

            paragraph.push('\n');
            paragraph.push_str(next_trimmed);
            lines.next();
        }

        blocks.push(MarkdownBlock::Paragraph(parse_inline(&paragraph)));
    }

    blocks
}

fn heading_text(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    rest.strip_prefix(' ').map(|text| (level, text))
}

fn bullet_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
}

fn ordered_bullet_text(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let digits = trimmed
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .count();

    if digits == 0 || !trimmed[digits..].starts_with(". ") {
        return None;
    }

    let number = trimmed[..digits].parse().ok()?;
    Some((number, &trimmed[digits + 2..]))
}

fn task_item_text(line: &str) -> Option<(bool, &str)> {
    let after_marker = bullet_text(line)?;
    if let Some(rest) = after_marker.strip_prefix("[ ] ") {
        Some((false, rest))
    } else if let Some(rest) = after_marker
        .strip_prefix("[x] ")
        .or_else(|| after_marker.strip_prefix("[X] "))
    {
        Some((true, rest))
    } else {
        None
    }
}

fn blockquote_text(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('>').map(str::trim_start)
}

/// Vrai si la ligne est une ligne de séparation de tableau : chaque cellule
/// vaut `---`, `:---`, `---:` ou `:---:` (au moins un tiret).
fn is_table_delimiter(line: &str) -> bool {
    if !line.contains('-') {
        return false;
    }
    let cells = split_table_cells(line);
    !cells.is_empty() && cells.iter().all(|cell| is_delimiter_cell(cell))
}

fn is_delimiter_cell(cell: &str) -> bool {
    let cell = cell.trim();
    let cell = cell.strip_prefix(':').unwrap_or(cell);
    let cell = cell.strip_suffix(':').unwrap_or(cell);
    !cell.is_empty() && cell.chars().all(|character| character == '-')
}

fn parse_table_alignments(line: &str) -> Vec<ColumnAlignment> {
    split_table_cells(line)
        .iter()
        .map(|cell| {
            let cell = cell.trim();
            match (cell.starts_with(':'), cell.ends_with(':')) {
                (true, true) => ColumnAlignment::Center,
                (false, true) => ColumnAlignment::Right,
                (true, false) => ColumnAlignment::Left,
                (false, false) => ColumnAlignment::None,
            }
        })
        .collect()
}

/// Parse une ligne de tableau en cellules, normalisée à `columns` colonnes
/// (comble avec du vide, tronque le surplus — la ligne de séparation fait foi).
fn parse_table_row(line: &str, columns: usize) -> Vec<Vec<MarkdownSpan>> {
    let mut cells: Vec<Vec<MarkdownSpan>> = split_table_cells(line)
        .iter()
        .map(|cell| parse_inline(cell.trim()))
        .collect();
    cells.truncate(columns);
    while cells.len() < columns {
        cells.push(Vec::new());
    }
    cells
}

/// Découpe une ligne sur les `|` non échappés, en retirant les barres
/// externes optionnelles. `\|` produit un `|` littéral dans la cellule.
fn split_table_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let body = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let body = if body.ends_with('|') && !body.ends_with("\\|") {
        &body[..body.len() - 1]
    } else {
        body
    };

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = body.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\\' if chars.peek() == Some(&'|') => {
                current.push('|');
                chars.next();
            }
            '|' => cells.push(std::mem::take(&mut current)),
            _ => current.push(character),
        }
    }
    cells.push(current);
    cells
}

fn is_thematic_break(line: &str) -> bool {
    let compact = line
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    compact.len() >= 3
        && compact
            .chars()
            .all(|character| matches!(character, '-' | '*' | '_'))
        && compact
            .chars()
            .all(|character| character == compact.chars().next().unwrap())
}

fn starts_special_block(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.is_empty()
        || fenced_code_language(line).is_some()
        || heading_text(line).is_some()
        || bullet_text(line).is_some()
        || ordered_bullet_text(line).is_some()
        || blockquote_text(line).is_some()
        || is_thematic_break(trimmed)
}

fn push_pending_blank(blocks: &mut Vec<MarkdownBlock>) {
    if matches!(blocks.last(), Some(MarkdownBlock::Blank)) {
        blocks.pop();
    }
}

fn fenced_code_language(line: &str) -> Option<Option<String>> {
    let rest = line.trim_start().strip_prefix("```")?;
    let language = rest.trim();
    if language.contains('`') {
        return None;
    }
    if language.is_empty() {
        Some(None)
    } else {
        Some(Some(language.to_string()))
    }
}

fn is_code_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

fn trim_trailing_blank_lines(code: &mut String) {
    while code.ends_with('\n') {
        code.pop();
    }

    loop {
        let Some(last_line_start) = code.rfind('\n') else {
            if code.trim().is_empty() {
                code.clear();
            }
            return;
        };

        if code[last_line_start + 1..].trim().is_empty() {
            code.truncate(last_line_start);
        } else {
            return;
        }
    }
}

fn parse_inline(input: &str) -> Vec<MarkdownSpan> {
    let mut spans = Vec::new();
    let mut rest = input;

    while !rest.is_empty() {
        if let Some((label, url, consumed)) = parse_link(rest) {
            spans.push(MarkdownSpan::Link {
                label: label.to_string(),
                url: url.to_string(),
            });
            rest = &rest[consumed..];
            continue;
        }

        if let Some((code, consumed)) = parse_code_span(rest) {
            spans.push(MarkdownSpan::Code(code.to_string()));
            rest = &rest[consumed..];
            continue;
        }

        if let Some(strike) = rest.strip_prefix("~~") {
            if let Some(end) = strike.find("~~") {
                spans.push(MarkdownSpan::Strikethrough(strike[..end].to_string()));
                rest = &strike[end + 2..];
                continue;
            } else {
                push_text(&mut spans, rest);
                break;
            }
        }

        if let Some((marker, strong)) = strong_text(rest) {
            if let Some(end) = strong.find(marker) {
                spans.push(MarkdownSpan::Strong(strong[..end].to_string()));
                rest = &strong[end + marker.len()..];
                continue;
            } else {
                push_text(&mut spans, rest);
                break;
            }
        }

        if let Some((marker, emphasis)) = emphasis_text(rest) {
            if let Some(end) = emphasis.find(marker) {
                spans.push(MarkdownSpan::Emphasis(emphasis[..end].to_string()));
                rest = &emphasis[end + marker.len()..];
                continue;
            } else {
                push_text(&mut spans, rest);
                break;
            }
        }

        let after_first_char = rest
            .char_indices()
            .nth(1)
            .map(|(idx, _)| idx)
            .unwrap_or(rest.len());
        let next_marker = ["`", "~~", "**", "__", "*", "_", "["]
            .iter()
            .filter_map(|marker| {
                rest[after_first_char..]
                    .find(marker)
                    .map(|idx| idx + after_first_char)
            })
            .min()
            .unwrap_or(rest.len());
        push_text(&mut spans, &rest[..next_marker]);
        rest = &rest[next_marker..];
    }

    spans
}

fn parse_code_span(input: &str) -> Option<(&str, usize)> {
    let delimiter_len = input
        .chars()
        .take_while(|&character| character == '`')
        .count();
    if delimiter_len == 0 {
        return None;
    }

    let delimiter = "`".repeat(delimiter_len);
    let rest = &input[delimiter_len..];
    let end = rest.find(&delimiter)?;
    Some((&rest[..end], delimiter_len + end + delimiter_len))
}

fn parse_link(input: &str) -> Option<(&str, &str, usize)> {
    let rest = input.strip_prefix('[')?;
    let label_end = rest.find("](")?;
    let after_label = &rest[label_end + 2..];
    let url_end = after_label.find(')')?;
    let label = &rest[..label_end];
    let url = &after_label[..url_end];
    let consumed = 1 + label_end + 2 + url_end + 1;
    Some((label, url, consumed))
}

fn strong_text(input: &str) -> Option<(&'static str, &str)> {
    if let Some(strong) = input.strip_prefix("**") {
        Some(("**", strong))
    } else if let Some(strong) = input.strip_prefix("__") {
        Some(("__", strong))
    } else {
        None
    }
}

fn emphasis_text(input: &str) -> Option<(&'static str, &str)> {
    if let Some(emphasis) = input.strip_prefix('*') {
        Some(("*", emphasis))
    } else if let Some(emphasis) = input.strip_prefix('_') {
        Some(("_", emphasis))
    } else {
        None
    }
}

fn push_text(spans: &mut Vec<MarkdownSpan>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(MarkdownSpan::Text(previous)) = spans.last_mut() {
        previous.push_str(text);
    } else {
        spans.push(MarkdownSpan::Text(text.to_string()));
    }
}

/// Résultat du parse d'un message, mis en cache par le fil (le parse ne se
/// fait qu'une fois par message, pas à chaque frame).
#[derive(Clone, Debug)]
pub(crate) struct ParsedMarkdown {
    pub(crate) blocks: Vec<MarkdownBlock>,
    pub(crate) emoji_only: bool,
}

/// Parse un message : blocs markdown + détection « uniquement des emojis »
/// (affichés en grand dans ce cas, façon Discord).
pub(crate) fn parse_message(
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
) -> ParsedMarkdown {
    ParsedMarkdown {
        blocks: parse_markdown(text),
        emoji_only: is_text_emoji_only(text, emoji_map),
    }
}

/// Rend des blocs déjà parsés (chemin chaud du fil : aucune allocation de
/// parse par frame).
pub(crate) fn render_parsed_markdown(
    ui: &mut egui::Ui,
    parsed: &ParsedMarkdown,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &super::EmojiTextures,
) {
    let emoji_size = if parsed.emoji_only { 44.0 } else { 22.0 };

    for block in &parsed.blocks {
        match block {
            MarkdownBlock::Blank => {
                ui.add_space(6.0);
            }
            MarkdownBlock::Paragraph(spans) => {
                ui.horizontal_wrapped(|ui| {
                    render_spans_with_emoji_size(
                        ui,
                        spans,
                        emoji_map,
                        emoji_textures,
                        None,
                        emoji_size,
                    )
                });
            }
            MarkdownBlock::Heading { level, spans } => {
                let size = match level {
                    1 => 22.0,
                    2 => 19.0,
                    _ => 17.0,
                };
                ui.horizontal_wrapped(|ui| {
                    render_spans_with_emoji_size(
                        ui,
                        spans,
                        emoji_map,
                        emoji_textures,
                        Some(SpanOverride::Heading(size)),
                        emoji_size,
                    );
                });
            }
            MarkdownBlock::Bullet(spans) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("• ");
                    render_spans_with_emoji_size(
                        ui,
                        spans,
                        emoji_map,
                        emoji_textures,
                        None,
                        emoji_size,
                    );
                });
            }
            MarkdownBlock::OrderedBullet { number, spans } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{}. ", number));
                    render_spans_with_emoji_size(
                        ui,
                        spans,
                        emoji_map,
                        emoji_textures,
                        None,
                        emoji_size,
                    );
                });
            }
            MarkdownBlock::TaskItem { checked, spans } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(if *checked { "☑ " } else { "☐ " });
                    render_spans_with_emoji_size(
                        ui,
                        spans,
                        emoji_map,
                        emoji_textures,
                        None,
                        emoji_size,
                    );
                });
            }
            MarkdownBlock::Blockquote(spans) => {
                render_blockquote_with_emoji_size(ui, spans, emoji_map, emoji_textures, emoji_size);
            }
            MarkdownBlock::CodeBlock { language, code } => {
                render_code_block(ui, language.as_deref(), code);
            }
            MarkdownBlock::Table {
                alignments,
                header,
                rows,
            } => {
                render_table(
                    ui,
                    alignments,
                    header,
                    rows,
                    emoji_map,
                    emoji_textures,
                    emoji_size,
                );
            }
            MarkdownBlock::ThematicBreak => {
                ui.separator();
            }
        }
    }
}

fn is_text_emoji_only(text: &str, emoji_map: &std::collections::HashMap<String, usize>) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if let Some((len, _)) = super::emoji_picker::match_emoji_at(&chars, i, emoji_map) {
            i += len;
            continue;
        }
        let ch = chars[i];
        if ch != '\u{fe0f}' && ch != '\u{200d}' && !ch.is_whitespace() {
            return false;
        }
        i += 1;
    }
    true
}

#[derive(Clone, Copy)]
enum SpanOverride {
    Heading(f32),
    Strong,
}

fn render_spans_with_emoji_size(
    ui: &mut egui::Ui,
    spans: &[MarkdownSpan],
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &super::EmojiTextures,
    override_style: Option<SpanOverride>,
    emoji_size: f32,
) {
    ui.spacing_mut().item_spacing.x = 0.0;
    for span in spans {
        match span {
            MarkdownSpan::Text(text) => match override_style {
                Some(SpanOverride::Heading(size)) => {
                    ui.label(egui::RichText::new(text).strong().size(size));
                }
                Some(SpanOverride::Strong) => {
                    ui.label(egui::RichText::new(text).strong());
                }
                None => {
                    super::emoji_picker::render_inline(
                        ui,
                        text,
                        emoji_map,
                        emoji_textures,
                        emoji_size,
                    );
                }
            },
            MarkdownSpan::Strong(text) => {
                let mut rich = egui::RichText::new(text).strong();
                if let Some(SpanOverride::Heading(size)) = override_style {
                    rich = rich.size(size);
                }
                ui.label(rich);
            }
            MarkdownSpan::Emphasis(text) => {
                let mut rich = egui::RichText::new(text).italics();
                if let Some(SpanOverride::Heading(size)) = override_style {
                    rich = rich.size(size).strong();
                }
                ui.label(rich);
            }
            MarkdownSpan::Strikethrough(text) => {
                let mut rich = egui::RichText::new(text).strikethrough();
                if let Some(SpanOverride::Heading(size)) = override_style {
                    rich = rich.size(size).strong();
                }
                ui.label(rich);
            }
            MarkdownSpan::Code(text) => {
                render_inline_code(ui, text);
            }
            MarkdownSpan::Link { label, url } => {
                ui.hyperlink_to(link_text(label, override_style), url);
            }
        }
    }
}

fn render_blockquote_with_emoji_size(
    ui: &mut egui::Ui,
    spans: &[MarkdownSpan],
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &super::EmojiTextures,
    emoji_size: f32,
) {
    let dark_mode = ui.visuals().dark_mode;
    let bg = if dark_mode {
        egui::Color32::from_rgb(30, 41, 59)
    } else {
        egui::Color32::from_rgb(241, 245, 249)
    };
    let border = if dark_mode {
        egui::Color32::from_rgb(125, 211, 252)
    } else {
        egui::Color32::from_rgb(3, 105, 161)
    };

    egui::Frame::NONE
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                render_spans_with_emoji_size(
                    ui,
                    spans,
                    emoji_map,
                    emoji_textures,
                    None,
                    emoji_size,
                );
            });
        });
}

fn render_inline_code(ui: &mut egui::Ui, text: &str) {
    let dark_mode = ui.visuals().dark_mode;
    let bg = if dark_mode {
        egui::Color32::from_rgb(30, 41, 59)
    } else {
        egui::Color32::from_rgb(226, 232, 240)
    };
    let text_color = if dark_mode {
        egui::Color32::from_rgb(248, 250, 252)
    } else {
        egui::Color32::from_rgb(15, 23, 42)
    };

    egui::Frame::NONE
        .fill(bg)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .monospace()
                    .color(text_color)
                    .size(13.0),
            );
        });
}

/// Espace horizontal entre deux colonnes.
const TABLE_COL_GAP: f32 = 8.0;

#[allow(clippy::too_many_arguments)]
fn render_table(
    ui: &mut egui::Ui,
    alignments: &[ColumnAlignment],
    header: &[Vec<MarkdownSpan>],
    rows: &[Vec<Vec<MarkdownSpan>>],
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &super::EmojiTextures,
    emoji_size: f32,
) {
    let dark_mode = ui.visuals().dark_mode;
    let border = if dark_mode {
        egui::Color32::from_rgb(71, 85, 105)
    } else {
        egui::Color32::from_rgb(148, 163, 184)
    };
    let header_bg = if dark_mode {
        egui::Color32::from_rgb(30, 41, 59)
    } else {
        egui::Color32::from_rgb(226, 232, 240)
    };
    let stripe_bg = if dark_mode {
        egui::Color32::from_rgb(23, 31, 46)
    } else {
        egui::Color32::from_rgb(241, 245, 249)
    };

    let columns = alignments.len().max(1);

    egui::Frame::NONE
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(6))
        .show(ui, |ui| {
            render_table_row(
                ui,
                header,
                alignments,
                columns,
                Some(header_bg),
                true,
                emoji_map,
                emoji_textures,
                emoji_size,
            );

            for (index, row) in rows.iter().enumerate() {
                let bg = if index % 2 == 1 {
                    Some(stripe_bg)
                } else {
                    None
                };
                render_table_row(
                    ui,
                    row,
                    alignments,
                    columns,
                    bg,
                    false,
                    emoji_map,
                    emoji_textures,
                    emoji_size,
                );
            }
        });
}

/// Marge horizontale interne d'une cellule (de chaque côté de la ligne).
const TABLE_ROW_PAD: f32 = 8.0;

#[allow(clippy::too_many_arguments)]
fn render_table_row(
    ui: &mut egui::Ui,
    cells: &[Vec<MarkdownSpan>],
    alignments: &[ColumnAlignment],
    columns: usize,
    background: Option<egui::Color32>,
    is_header: bool,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &super::EmojiTextures,
    emoji_size: f32,
) {
    let mut frame = egui::Frame::NONE.inner_margin(egui::Margin {
        left: TABLE_ROW_PAD as i8,
        right: TABLE_ROW_PAD as i8,
        top: 5,
        bottom: 5,
    });
    if let Some(bg) = background {
        frame = frame.fill(bg);
    }

    frame.show(ui, |ui| {
        // Largeur de colonne calculée sur la largeur de contenu RÉELLE de la
        // ligne (marges déjà déduites) : identique pour l'en-tête et le corps,
        // donc les colonnes restent alignées et le tableau ne déborde jamais.
        let avail = ui.available_width() - 0.5;
        let col_width =
            ((avail - TABLE_COL_GAP * (columns as f32 - 1.0)) / columns as f32).max(24.0);

        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            ui.spacing_mut().item_spacing.x = TABLE_COL_GAP;
            let empty = Vec::new();
            for column in 0..columns {
                let spans = cells.get(column).unwrap_or(&empty);
                let alignment = alignments
                    .get(column)
                    .copied()
                    .unwrap_or(ColumnAlignment::None);
                let cross = match alignment {
                    ColumnAlignment::Right => egui::Align::RIGHT,
                    ColumnAlignment::Center => egui::Align::Center,
                    ColumnAlignment::None | ColumnAlignment::Left => egui::Align::LEFT,
                };
                ui.allocate_ui_with_layout(
                    egui::vec2(col_width, 0.0),
                    egui::Layout::top_down(cross),
                    |ui| {
                        ui.set_width(col_width);
                        ui.horizontal_wrapped(|ui| {
                            render_spans_with_emoji_size(
                                ui,
                                spans,
                                emoji_map,
                                emoji_textures,
                                is_header.then_some(SpanOverride::Strong),
                                emoji_size,
                            );
                        });
                    },
                );
            }
        });
    });
}

fn render_code_block(ui: &mut egui::Ui, language: Option<&str>, code: &str) {
    let dark_mode = ui.visuals().dark_mode;
    let (bg, border, text_color, chip_bg, chip_text) = if dark_mode {
        (
            egui::Color32::from_rgb(15, 23, 42),
            egui::Color32::from_rgb(71, 85, 105),
            egui::Color32::from_rgb(248, 250, 252),
            egui::Color32::from_rgb(30, 41, 59),
            egui::Color32::from_rgb(125, 211, 252),
        )
    } else {
        (
            egui::Color32::from_rgb(241, 245, 249),
            egui::Color32::from_rgb(148, 163, 184),
            egui::Color32::from_rgb(15, 23, 42),
            egui::Color32::from_rgb(226, 232, 240),
            egui::Color32::from_rgb(3, 105, 161),
        )
    };

    egui::Frame::NONE
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width().max(180.0));

            if let Some(language) = language.filter(|language| !language.is_empty()) {
                egui::Frame::NONE
                    .fill(chip_bg)
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(language)
                                .small()
                                .strong()
                                .color(chip_text),
                        );
                    });
                ui.add_space(6.0);
            }

            egui::ScrollArea::horizontal()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(if code.is_empty() { " " } else { code })
                                .monospace()
                                .size(13.0)
                                .color(text_color),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                });
        });
}

fn link_text(label: &str, override_style: Option<SpanOverride>) -> egui::RichText {
    let mut rich = egui::RichText::new(label).underline();
    if let Some(SpanOverride::Heading(size)) = override_style {
        rich = rich.size(size).strong();
    }
    rich
}

#[cfg(test)]
#[path = "../tests/test_ui_markdown.rs"]
mod tests;
