use eframe::egui;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MarkdownBlock {
    Paragraph(Vec<MarkdownSpan>),
    Heading {
        level: usize,
        spans: Vec<MarkdownSpan>,
    },
    Bullet(Vec<MarkdownSpan>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Blank,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MarkdownSpan {
    Text(String),
    Strong(String),
    Emphasis(String),
    Code(String),
}

pub(crate) fn parse_markdown(input: &str) -> Vec<MarkdownBlock> {
    if input.is_empty() {
        return vec![MarkdownBlock::Paragraph(Vec::new())];
    }

    let mut blocks = Vec::new();
    let mut lines = input.lines();

    while let Some(line) = lines.next() {
        if let Some(language) = fenced_code_language(line) {
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

        if line.trim().is_empty() {
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

        if let Some(text) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            blocks.push(MarkdownBlock::Bullet(parse_inline(text)));
            continue;
        }

        blocks.push(MarkdownBlock::Paragraph(parse_inline(line)));
    }

    blocks
}

fn heading_text(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|&c| c == '#').count();
    if !(1..=3).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    rest.strip_prefix(' ').map(|text| (level, text))
}

fn fenced_code_language(line: &str) -> Option<Option<String>> {
    let rest = line.trim_start().strip_prefix("```")?;
    let language = rest.trim();
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
        if let Some(code) = rest.strip_prefix('`') {
            if let Some(end) = code.find('`') {
                spans.push(MarkdownSpan::Code(code[..end].to_string()));
                rest = &code[end + 1..];
                continue;
            } else {
                push_text(&mut spans, rest);
                break;
            }
        }

        if let Some(strong) = rest.strip_prefix("**") {
            if let Some(end) = strong.find("**") {
                spans.push(MarkdownSpan::Strong(strong[..end].to_string()));
                rest = &strong[end + 2..];
                continue;
            } else {
                push_text(&mut spans, rest);
                break;
            }
        }

        if let Some(emphasis) = rest.strip_prefix('*') {
            if let Some(end) = emphasis.find('*') {
                spans.push(MarkdownSpan::Emphasis(emphasis[..end].to_string()));
                rest = &emphasis[end + 1..];
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
        let next_marker = ["`", "**", "*"]
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
            MarkdownBlock::CodeBlock { language, code } => {
                render_code_block(ui, language.as_deref(), &code);
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
                ui.label(
                    egui::RichText::new(text)
                        .monospace()
                        .background_color(ui.visuals().widgets.noninteractive.bg_fill),
                );
            }
        }
    }
}

fn render_code_block(ui: &mut egui::Ui, _language: Option<&str>, code: &str) {
    let bg = egui::Color32::from_rgb(17, 24, 39);
    let border = egui::Color32::from_rgb(75, 85, 99);
    let text_color = egui::Color32::from_rgb(249, 250, 251);

    egui::Frame::NONE
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());

            let previous_spacing = ui.spacing().item_spacing;
            ui.spacing_mut().item_spacing.y = 0.0;
            for line in code.lines().chain((code.is_empty()).then_some("")) {
                ui.label(
                    egui::RichText::new(line)
                        .monospace()
                        .size(13.0)
                        .color(text_color),
                );
            }
            ui.spacing_mut().item_spacing = previous_spacing;
        });
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
}
