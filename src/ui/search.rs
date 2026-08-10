//! Recherche plein texte dans l'historique (Cmd/Ctrl+F).
//!
//! L'index vit dans SQLite (FTS5 sans copie du contenu) : l'UI n'envoie qu'une
//! requête au thread de stockage et affiche ce qui revient.

use super::i18n;
use eframe::egui;

use crate::app::AppState;
use crate::util::MutexExt;

use super::chat_panel::{header_time, peer_color_for};
use super::AbcomApp;

/// Longueur minimale avant d'interroger l'index : en dessous, toute
/// l'application ressortirait.
const MIN_QUERY_CHARS: usize = 2;

impl AbcomApp {
    pub(crate) fn show_search(&mut self, ctx: &egui::Context) {
        if !self.search.open {
            return;
        }

        let title = self.t(i18n::RECHERCHER);
        let hint = self.t(i18n::RECHERCHER_DANS_L_HISTORIQUE);
        let empty = self.t(i18n::AUCUN_RESULTAT);
        let short = self.t(i18n::TAPEZ_AU_MOINS_2_CARACTERES);
        let my_name = self.state.lock_safe().my_username.clone();

        let mut jump_to: Option<(Option<String>, u64)> = None;
        let modal = egui::Modal::new(egui::Id::new("search")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading(title);
            ui.add_space(6.0);

            let field = ui.add(
                egui::TextEdit::singleline(&mut self.search.query)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
            if std::mem::take(&mut self.search.focus_requested) {
                field.request_focus();
            }
            ui.add_space(8.0);

            if self.search.query.chars().count() < MIN_QUERY_CHARS {
                ui.label(egui::RichText::new(short).weak());
                return;
            }
            if self.search.results.is_empty() {
                ui.label(egui::RichText::new(empty).weak());
                return;
            }

            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for message in &self.search.results {
                        let conversation = conversation_of(message, &my_name);
                        let response = ui
                            .add(
                                egui::Label::new(result_line(
                                    message,
                                    &my_name,
                                    ui.visuals().dark_mode,
                                ))
                                .sense(egui::Sense::click()),
                            )
                            .on_hover_text(conversation_label(&conversation));
                        if response.clicked() {
                            jump_to = Some((conversation, AppState::message_hash(message)));
                        }
                        ui.separator();
                    }
                });
        });

        if modal.backdrop_response.clicked() {
            self.search.open = false;
        }
        if let Some((conversation, hash)) = jump_to {
            self.switch_conversation(conversation);
            self.scroll_to_message = Some(hash);
            self.highlight_message = Some((hash, std::time::Instant::now()));
            self.search.open = false;
        }
    }

    /// Envoie la requête au stockage quand elle a changé (anti-rebond par
    /// comparaison : inutile de relancer la même recherche à chaque frame).
    pub(crate) fn submit_search(&mut self) {
        if !self.search.open {
            return;
        }
        let query = self.search.query.trim().to_string();
        if query == self.search.submitted {
            return;
        }
        self.search.submitted = query.clone();
        if query.chars().count() < MIN_QUERY_CHARS {
            self.search.results.clear();
            return;
        }
        self.state.lock_safe().search_history(query);
    }
}

/// Conversation d'où provient un message, du point de vue du destinataire.
fn conversation_of(message: &crate::message::ChatMessage, me: &str) -> Option<String> {
    match message.to_user.as_deref() {
        None => None,
        Some(room) if room.starts_with('#') => Some(room.to_string()),
        Some(_) if message.from == me => message.to_user.clone(),
        Some(_) => Some(message.from.clone()),
    }
}

fn conversation_label(conversation: &Option<String>) -> String {
    conversation.clone().unwrap_or_else(|| "Tous".to_string())
}

fn result_line(message: &crate::message::ChatMessage, me: &str, dark_mode: bool) -> egui::RichText {
    let author = if message.from == me {
        "Vous"
    } else {
        &message.from
    };
    let extract: String = message.content.chars().take(140).collect();
    egui::RichText::new(format!(
        "{}  {}  —  {extract}",
        header_time(message),
        author
    ))
    .color(peer_color_for(&message.from, dark_mode))
}
