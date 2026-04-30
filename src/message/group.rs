use serde::{Deserialize, Serialize};

/// Représente un groupe de chat
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Group {
    pub name: String,
    pub owner: String,
    pub members: Vec<String>,
    pub created_at: String,
}

/// Événement de synchronisation de groupe envoyé par TCP
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GroupEvent {
    pub action: GroupAction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GroupAction {
    Create { group: Group },
    AddMember { group_name: String, username: String },
    RemoveMember { group_name: String, username: String },
    Rename { group_name: String, new_name: String },
    Delete { group_name: String },
}
