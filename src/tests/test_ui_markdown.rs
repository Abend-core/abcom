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
            MarkdownBlock::Blockquote(vec![MarkdownSpan::Text("note importante".to_string(),)]),
        ]
    );
}

#[test]
fn keeps_newlines_between_plain_lines() {
    // Sémantique chat : Entrée = retour à la ligne conservé dans le rendu.
    assert_eq!(
        parse_markdown("ligne un\nligne deux\n\n- suite"),
        vec![
            MarkdownBlock::Paragraph(vec![
                MarkdownSpan::Text("ligne un\nligne deux".to_string(),)
            ]),
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
