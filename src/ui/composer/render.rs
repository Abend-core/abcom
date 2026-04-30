use eframe::egui;

/// Rend un texte contenant des emojis sous forme de séquence Label + Image
pub fn render_inline(
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
        for len in [2usize, 1usize] {
            if i + len <= chars.len() {
                let s: String = chars[i..i + len].iter().collect();
                if let Some(&idx) = emoji_map.get(&s) {
                    if !acc.is_empty() { ui.label(&acc); acc.clear(); }
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
            if ch != '\u{fe0f}' && ch != '\u{200d}' { acc.push(ch); }
            i += 1;
        }
    }
    if !acc.is_empty() { ui.label(&acc); }
}

/// Mesure la largeur d'un texte en pixels avec la police donnée
pub fn measure_text_width(text: &str, ctx: &egui::Context, font_id: &egui::FontId) -> f32 {
    ctx.fonts(|fonts| {
        fonts.layout_no_wrap(text.to_string(), font_id.clone(), egui::Color32::WHITE).rect.width()
    })
}
