use serde::{Deserialize, Serialize};

/// Représente un groupe de chat.
///
/// L'identité d'un salon est son [`Group::id`], pas son nom : `to_user` porte
/// cet identifiant et il entre dans le hash des messages. Le nom n'est qu'un
/// libellé d'affichage, librement modifiable sans invalider un seul hash,
/// accusé, réaction ou repère de lecture.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Group {
    /// Identifiant immuable. `#[serde(default)]` : les salons créés avant son
    /// introduction arrivent sans, et retombent sur [`Group::derived_id`].
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub owner: String,
    pub members: Vec<String>,
    pub created_at: String,
}

impl Group {
    /// Identifiant d'un salon antérieur au champ `id`, reconstruit à partir de
    /// ses seules données immuables.
    ///
    /// Le `Group` est répliqué tel quel par `GroupAction::Create` : chaque pair
    /// dérive donc la même valeur sans échange supplémentaire, ce qui est
    /// indispensable — l'émetteur et le destinataire doivent calculer le même
    /// hash de message.
    pub fn derived_id(owner: &str, created_at: &str, name: &str) -> String {
        let hash = super::chat::fnv1a(format!("{owner}|{created_at}|{name}").as_bytes());
        format!("g{hash:016x}")
    }

    /// Identifiant effectif : celui du salon, ou sa dérivation si absent.
    pub fn effective_id(&self) -> String {
        if self.id.is_empty() {
            Self::derived_id(&self.owner, &self.created_at, &self.name)
        } else {
            self.id.clone()
        }
    }

    /// Fixe l'identifiant s'il manque (chargement d'un salon hérité).
    pub fn ensure_id(&mut self) {
        if self.id.is_empty() {
            self.id = Self::derived_id(&self.owner, &self.created_at, &self.name);
        }
    }
}

/// Événement de synchronisation de groupe envoyé par TCP
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GroupEvent {
    pub action: GroupAction,
}

/// Les salons sont désignés par leur identifiant : un renommage concurrent ne
/// peut plus faire manquer sa cible à un événement en vol.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GroupAction {
    Create { group: Group },
    AddMember { group_id: String, username: String },
    RemoveMember { group_id: String, username: String },
    Rename { group_id: String, new_name: String },
    Delete { group_id: String },
}

#[cfg(test)]
#[path = "../tests/test_message_group.rs"]
mod tests;
