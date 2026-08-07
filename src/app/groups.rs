use std::net::SocketAddr;

use super::AppState;
use crate::message::Group;

impl AppState {
    /// Persiste la liste des groupes (remplacement complet, table petite)
    /// et invalide les caches dérivés de l'UI (barre latérale, fil).
    pub fn save_groups(&mut self) {
        self.persist(super::StorageCmd::ReplaceGroups(self.groups.clone()));
        self.bump_content();
    }

    /// Clé de conversation d'un salon : `to_user` porte `#<nom>` pour les
    /// messages de groupe (les noms de pairs ne peuvent pas contenir `#`).
    pub fn group_conv_key(group_name: &str) -> String {
        format!("#{group_name}")
    }

    fn validate_group_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 50
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }

    pub fn create_group(&mut self, name: String, members: Vec<String>) -> Option<Group> {
        let name = name.trim().to_string();
        if !Self::validate_group_name(&name) {
            return None;
        }
        if self
            .groups
            .iter()
            .any(|g| g.name.eq_ignore_ascii_case(&name))
        {
            return None;
        }
        let invalid: Vec<_> = members
            .iter()
            .filter(|m| !self.peers.iter().any(|p| p.username == **m) && **m != self.my_username)
            .collect();
        if !invalid.is_empty() {
            return None;
        }

        let mut group_members = vec![self.my_username.clone()];
        for m in members {
            if m != self.my_username && !group_members.contains(&m) {
                group_members.push(m);
            }
        }
        let group = Group {
            name: name.clone(),
            owner: self.my_username.clone(),
            members: group_members,
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };
        self.groups.push(group.clone());
        self.save_groups();
        Some(group)
    }

    pub fn add_member_to_group(&mut self, group_name: &str, username: String) -> bool {
        let known_peer =
            username == self.my_username || self.peers.iter().any(|p| p.username == username);
        if let Some(g) = self.groups.iter_mut().find(|g| g.name == group_name) {
            if g.owner == self.my_username && known_peer && !g.members.contains(&username) {
                g.members.push(username);
                self.save_groups();
                return true;
            }
        }
        false
    }

    /// Retrait d'un membre par le propriétaire (jamais lui-même : il passe
    /// par `leave_group`, avec succession).
    pub fn remove_member_from_group(&mut self, group_name: &str, username: &str) -> bool {
        let allowed = self
            .groups
            .iter()
            .any(|g| g.name == group_name && g.owner == self.my_username && username != g.owner);
        if !allowed {
            return false;
        }
        self.apply_member_removal(group_name, username);
        true
    }

    /// Applique le retrait d'un membre (départ volontaire, exclusion, ou
    /// événement réseau), avec la règle de succession : si le propriétaire
    /// part, le premier membre restant (ordre d'arrivée) hérite du groupe ;
    /// s'il ne reste personne, le groupe disparaît. Si le membre retiré est
    /// l'utilisateur local, le salon et son historique local disparaissent.
    pub fn apply_member_removal(&mut self, group_name: &str, username: &str) {
        if username == self.my_username {
            self.leave_group(group_name);
            return;
        }
        let Some(idx) = self.groups.iter().position(|g| g.name == group_name) else {
            return;
        };
        {
            let g = &mut self.groups[idx];
            g.members.retain(|m| m != username);
            if g.owner == username {
                if let Some(next) = g.members.first() {
                    g.owner = next.clone();
                }
            }
        }
        if self.groups[idx].members.is_empty() {
            self.groups.remove(idx);
        }
        self.save_groups();
    }

    /// Quitte un groupe : il disparaît de la liste et l'historique local du
    /// salon est effacé (politique documentée dans docs/05-fonctionnalites.md — les
    /// autres membres conservent les messages, attribués à leur auteur).
    pub fn leave_group(&mut self, group_name: &str) -> bool {
        let before = self.groups.len();
        self.groups.retain(|g| g.name != group_name);
        if self.groups.len() == before {
            return false;
        }
        self.forget_group_conversation(group_name);
        self.save_groups();
        true
    }

    /// Efface localement le fil d'un salon disparu (départ, exclusion,
    /// suppression) : messages en mémoire et en base, compteur de lecture,
    /// retour au fil « Tous » si le salon était ouvert.
    fn forget_group_conversation(&mut self, group_name: &str) {
        let conv = Self::group_conv_key(group_name);
        self.messages
            .retain(|m| m.to_user.as_deref() != Some(conv.as_str()));
        self.read_counts.remove(&conv);
        self.persist(super::StorageCmd::DeleteConversation {
            me: self.my_username.clone(),
            conv: Some(conv.clone()),
        });
        if self.selected_conversation.as_deref() == Some(conv.as_str()) {
            self.selected_conversation = None;
        }
        self.bump_content();
    }

    /// Entrée locale (avec vérification de propriétaire) pour renommer un
    /// salon dont on est propriétaire, puis diffuser l'événement.
    /// Non branchée : il n'existe pas encore de déclencheur UI pour un
    /// renommage local (seul `apply_group_rename` est utilisé, côté
    /// réception réseau). Gap fonctionnel identifié, pas du code mort à
    /// supprimer — câblage d'une UI de renommage hors périmètre ici.
    #[allow(dead_code)]
    pub fn rename_group(&mut self, group_name: &str, new_name: String) -> bool {
        if !self.is_group_owner(group_name) {
            return false;
        }
        self.apply_group_rename(group_name, new_name)
    }

    /// Applique un renommage (local ou reçu du propriétaire) : valide le
    /// nouveau nom, migre l'historique (`to_user`) et le compteur de lecture
    /// vers la nouvelle clé de conversation.
    pub fn apply_group_rename(&mut self, group_name: &str, new_name: String) -> bool {
        let new_name = new_name.trim().to_string();
        if !Self::validate_group_name(&new_name) {
            return false;
        }
        if self
            .groups
            .iter()
            .any(|g| g.name.eq_ignore_ascii_case(&new_name) && g.name != group_name)
        {
            return false;
        }
        let Some(g) = self.groups.iter_mut().find(|g| g.name == group_name) else {
            return false;
        };
        if g.name == new_name {
            return true;
        }
        g.name = new_name.clone();

        let old_conv = Self::group_conv_key(group_name);
        let new_conv = Self::group_conv_key(&new_name);
        for m in self.messages.iter_mut() {
            if m.to_user.as_deref() == Some(old_conv.as_str()) {
                m.to_user = Some(new_conv.clone());
            }
        }
        if let Some(read) = self.read_counts.remove(&old_conv) {
            self.read_counts.insert(new_conv.clone(), read);
            self.persist(super::StorageCmd::SetReadCount {
                username: new_conv.clone(),
                count: read as u64,
            });
        }
        self.persist(super::StorageCmd::RenameConversation {
            old: old_conv.clone(),
            new: new_conv.clone(),
        });
        if self.selected_conversation.as_deref() == Some(old_conv.as_str()) {
            self.selected_conversation = Some(new_conv);
        }
        self.save_groups();
        true
    }

    /// Supprime un groupe dont nous sommes propriétaire, historique local
    /// compris. Les membres reçoivent l'événement `Delete` (envoyé par l'UI).
    pub fn delete_group(&mut self, group_name: &str) -> bool {
        if let Some(pos) = self
            .groups
            .iter()
            .position(|g| g.name == group_name && g.owner == self.my_username)
        {
            self.groups.remove(pos);
            self.forget_group_conversation(group_name);
            self.save_groups();
            return true;
        }
        false
    }

    /// Supprime un groupe sur ordre du réseau (événement `Delete` émis par le
    /// propriétaire), historique local compris.
    pub fn apply_group_delete(&mut self, group_name: &str) {
        let before = self.groups.len();
        self.groups.retain(|g| g.name != group_name);
        if self.groups.len() != before {
            self.forget_group_conversation(group_name);
            self.save_groups();
        }
    }

    pub fn get_group(&self, group_name: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.name == group_name)
    }

    pub fn is_group_owner(&self, group_name: &str) -> bool {
        self.groups
            .iter()
            .any(|g| g.name == group_name && g.owner == self.my_username)
    }

    pub fn is_in_group(&self, group_name: &str) -> bool {
        self.groups
            .iter()
            .any(|g| g.name == group_name && g.members.contains(&self.my_username))
    }

    /// Adresses des membres du groupe actuellement en ligne (moi exclu) :
    /// destinataires des messages du salon et des événements de groupe.
    pub fn group_member_addrs(&self, group_name: &str) -> Vec<SocketAddr> {
        self.group_member_recipients(group_name)
            .into_iter()
            .map(|(_, addr)| addr)
            .collect()
    }

    pub(crate) fn group_member_recipients(&self, group_name: &str) -> Vec<(String, SocketAddr)> {
        let Some(g) = self.get_group(group_name) else {
            return Vec::new();
        };
        g.members
            .iter()
            .filter(|m| **m != self.my_username)
            .filter_map(|m| {
                self.peers
                    .iter()
                    .find(|p| p.online && p.username == *m && !p.addr.ip().is_unspecified())
                    .map(|p| (p.username.clone(), p.addr))
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "../tests/test_app_groups.rs"]
mod tests;
