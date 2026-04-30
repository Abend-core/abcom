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
    Blockquote(Vec<MarkdownSpan>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    ThematicBreak,
    Blank,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MarkdownSpan {
    Text(String),
    Strong(String),
    Emphasis(String),
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

        if let Some((level, text)) = heading_text(line) {
            blocks.push(MarkdownBlock::Heading {
                level,
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

        let mut paragraph = trimmed.to_string();
        while let Some(next_line) = lines.peek().copied() {
            let next_trimmed = next_line.trim();
            if next_trimmed.is_empty() || starts_special_block(next_line) {
                break;
            }

            let separator = if line_ends_with_hard_break(paragraph.as_str()) {
                "\n"
            } else {
                " "
            };
            paragraph.push_str(separator);
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

fn blockquote_text(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('>').map(str::trim_start)
}

fn is_thematic_break(line: &str) -> bool {
    let compact = line.chars().filter(|character| !character.is_whitespace()).collect::<String>();
    compact.len() >= 3
        && compact
            .chars()
            .all(|character| matches!(character, '-' | '*' | '_'))
        && compact.chars().all(|character| character == compact.chars().next().unwrap())
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

fn line_ends_with_hard_break(line: &str) -> bool {
    line.ends_with("  ")
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
        let next_marker = ["`", "**", "__", "*", "_", "["]
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
    let delimiter_len = input.chars().take_while(|&character| character == '`').count();
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

pub(crate) fn render_message_markdown(
    ui: &mut egui::Ui,
    text: &str,
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
) {
    for block in parse_markdown(text) {
        match block {
            MarkdownBlock::Blank => {
                ui.add_space(6.0);
            }
            MarkdownBlock::Paragraph(spans) => {
                ui.horizontal_wrapped(|ui| {
                    render_spans(ui, &spans, emoji_map, emoji_textures, None)
                });
            }
            MarkdownBlock::Heading { level, spans } => {
                let size = match level {
                    1 => 22.0,
                    2 => 19.0,
                    _ => 17.0,
                };
                ui.horizontal_wrapped(|ui| {
                    render_spans(
                        ui,
                        &spans,
                        emoji_map,
                        emoji_textures,
                        Some(SpanOverride::Heading(size)),
                    );
                });
            }
            MarkdownBlock::Bullet(spans) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label("• ");
                    render_spans(ui, &spans, emoji_map, emoji_textures, None);
                });
            }
            MarkdownBlock::OrderedBullet { number, spans } => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{}. ", number));
                    render_spans(ui, &spans, emoji_map, emoji_textures, None);
                });
            }
            MarkdownBlock::Blockquote(spans) => {
                render_blockquote(ui, &spans, emoji_map, emoji_textures);
            }
            MarkdownBlock::CodeBlock { language, code } => {
                render_code_block(ui, language.as_deref(), &code);
            }
            MarkdownBlock::ThematicBreak => {
                ui.separator();
            }
        }
    }
}

#[derive(Clone, Copy)]
enum SpanOverride {
    Heading(f32),
}

fn render_spans(
    ui: &mut egui::Ui,
    spans: &[MarkdownSpan],
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
    override_style: Option<SpanOverride>,
) {
    ui.spacing_mut().item_spacing.x = 0.0;
    for span in spans {
        match span {
            MarkdownSpan::Text(text) => {
                if let Some(SpanOverride::Heading(size)) = override_style {
                    ui.label(egui::RichText::new(text).strong().size(size));
                } else {
                    super::emoji_picker::render_inline(ui, text, emoji_map, emoji_textures, 16.0);
                }
            }
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
            MarkdownSpan::Code(text) => {
                render_inline_code(ui, text);
            }
            MarkdownSpan::Link { label, url } => {
                ui.hyperlink_to(link_text(label, override_style), url);
            }
        }
    }
}

fn render_blockquote(
    ui: &mut egui::Ui,
    spans: &[MarkdownSpan],
    emoji_map: &std::collections::HashMap<String, usize>,
    emoji_textures: &[(String, egui::TextureHandle)],
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
                render_spans(ui, spans, emoji_map, emoji_textures, None);
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
mod tests {
    use super::*;

    #[test]
    fn parses_bold_italic_and_code_spans() {
        assert_eq!(
            parse_markdown("Un **gras**, un *italique* et `code`"),
            vec![MarkdownBlock::Paragraph(vec![
                MarkdownSpan::Text("Un ".to_string()),
                MarkdownSpan::Strong("gras".to_string()),
                MarkdownSpan::Text(", un ".to_string()),
                MarkdownSpan::Emphasis("italique".to_string()),
                MarkdownSpan::Text(" et ".to_string()),
                MarkdownSpan::Code("code".to_string()),
            ])]
        );
    }

    #[test]
    fn parses_headings_and_bullets() {
        assert_eq!(
            parse_markdown("# Titre\n- item **important**"),
            vec![
                MarkdownBlock::Heading {
                    level: 1,
                    spans: vec![MarkdownSpan::Text("Titre".to_string())],
                },
                MarkdownBlock::Bullet(vec![
                    MarkdownSpan::Text("item ".to_string()),
                    MarkdownSpan::Strong("important".to_string()),
                ]),
            ]
        );
    }

    #[test]
    fn parses_ordered_quotes_and_links() {
        assert_eq!(
            parse_markdown("1. [Guide](https://example.com)\n> note importante"),
            vec![
                MarkdownBlock::OrderedBullet {
                    number: 1,
                    spans: vec![MarkdownSpan::Link {
                        label: "Guide".to_string(),
                        url: "https://example.com".to_string(),
                    }],
                },
                MarkdownBlock::Blockquote(vec![MarkdownSpan::Text(
                    "note importante".to_string(),
                )]),
            ]
        );
    }

    #[test]
    fn merges_plain_lines_into_single_paragraph() {
        assert_eq!(
            parse_markdown("ligne un\nligne deux\n\n- suite"),
            vec![
                MarkdownBlock::Paragraph(vec![MarkdownSpan::Text(
                    "ligne un ligne deux".to_string(),
                )]),
                MarkdownBlock::Blank,
                MarkdownBlock::Bullet(vec![MarkdownSpan::Text("suite".to_string())]),
            ]
        );
    }

    #[test]
    fn parses_fenced_code_blocks() {
        assert_eq!(
            parse_markdown("Avant\n```rust\nfn main() {\n    println!(\"ok\");\n}\n```\nApres"),
            vec![
                MarkdownBlock::Paragraph(vec![MarkdownSpan::Text("Avant".to_string())]),
                MarkdownBlock::CodeBlock {
                    language: Some("rust".to_string()),
                    code: "fn main() {\n    println!(\"ok\");\n}".to_string(),
                },
                MarkdownBlock::Paragraph(vec![MarkdownSpan::Text("Apres".to_string())]),
            ]
        );
    }

    #[test]
    fn keeps_unclosed_fenced_code_as_code_block() {
        assert_eq!(
            parse_markdown("```\ncode **non markdown**"),
            vec![MarkdownBlock::CodeBlock {
                language: None,
                code: "code **non markdown**".to_string(),
            }]
        );
    }

    #[test]
    fn trims_trailing_blank_lines_from_fenced_code() {
        assert_eq!(
            parse_markdown("```rust\nfn main() {}\n\n```"),
            vec![MarkdownBlock::CodeBlock {
                language: Some("rust".to_string()),
                code: "fn main() {}".to_string(),
            }]
        );
    }

    #[test]
    fn leaves_unclosed_markers_as_text() {
        assert_eq!(
            parse_markdown("hello **pas ferme"),
            vec![MarkdownBlock::Paragraph(vec![MarkdownSpan::Text(
                "hello **pas ferme".to_string()
            )])]
        );
    }

    #[test]
    fn keeps_single_line_triple_backticks_as_inline_code() {
        assert_eq!(
            parse_markdown("``` test ```"),
            vec![MarkdownBlock::Paragraph(vec![MarkdownSpan::Code(
                " test ".to_string(),
            )])]
        );
    }
}
