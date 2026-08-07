use eframe::egui;

use crate::message::ReactionRequest;
use crate::util::MutexExt;

use super::AbcomApp;

/// Nombre maximal d'emojis récents conservés dans la barre de survol.
const MAX_RECENT_EMOJIS: usize = 6;

/// Déplace `emoji` en tête de `recent` (ou l'y insère), et tronque à `max_len`.
/// Fonction pure, testée indépendamment de l'UI.
fn update_recent_emojis(recent: &mut Vec<String>, emoji: &str, max_len: usize) {
    recent.retain(|e| e != emoji);
    recent.insert(0, emoji.to_string());
    recent.truncate(max_len);
}

impl AbcomApp {
    /// Bascule la réaction de l'utilisateur courant sur `message_hash` pour
    /// `emoji` (ajout si absente, retrait si déjà présente), diffuse
    /// l'événement net aux pairs concernés, et met à jour les emojis récents.
    pub(crate) fn send_reaction(&mut self, message_hash: u64, emoji: &str) {
        let (my_name, action, targets) = {
            let mut s = self.state.lock_safe();
            let my_name = s.my_username.clone();
            let action = s.toggle_reaction(message_hash, emoji, &my_name);
            (my_name, action, s.selected_transfer_targets())
        };
        update_recent_emojis(&mut self.recent_reaction_emojis, emoji, MAX_RECENT_EMOJIS);
        let event = crate::message::ReactionEvent {
            message_hash,
            emoji: emoji.to_string(),
            user: my_name,
            action,
        };
        for target in targets {
            self.net.try_send(ReactionRequest {
                to_peer: target.username,
                to_addr: target.addr,
                event: event.clone(),
            });
        }
    }

    /// Popup flottante du picker emoji dédié aux réactions, ancrée près du
    /// bouton "+" de la barre de survol (contrairement au picker du
    /// composeur, ancré en bas-droite de l'écran).
    pub(crate) fn show_reaction_emoji_picker(&mut self, ctx: &egui::Context) {
        let Some((target_hash, anchor)) = self.reaction_picker_open else {
            return;
        };

        let popup_id = egui::Id::new("reaction_emoji_picker");
        let popup_pos = anchor.left_bottom() + egui::vec2(0.0, 6.0);

        let mut picked: Option<String> = None;
        let area = egui::Area::new(popup_id)
            .order(egui::Order::Foreground)
            .fixed_pos(popup_pos);
        let resp = area.show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_size(egui::vec2(310.0, 340.0));
                super::emoji_picker::show_emoji_grid(
                    ui,
                    &mut self.emoji.category,
                    &self.emoji.textures,
                    |ch| picked = Some(ch.to_string()),
                );
            });
        });
        let picker_rect = resp.response.rect;

        if let Some(emoji) = picked {
            self.send_reaction(target_hash, &emoji);
            self.reaction_picker_open = None;
            return;
        }

        // Ferme au clic en dehors du picker et du bouton "+" qui l'a ouvert.
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if !picker_rect.contains(pos) && !anchor.contains(pos) {
                    self.reaction_picker_open = None;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/test_ui_reactions.rs"]
mod tests;
