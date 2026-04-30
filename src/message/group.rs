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

#[cfg(test)]
mod tests {
    use super::{Group, GroupAction, GroupEvent};

    fn make_group(name: &str, owner: &str) -> Group {
        Group {
            name: name.to_string(),
            owner: owner.to_string(),
            members: vec![owner.to_string()],
            created_at: "2026-01-01 12:00:00".to_string(),
        }
    }

    // ── Group sérialisation ─────────────────────────────────────────────────

    #[test]
    fn group_round_trip() {
        let g = make_group("DevTeam", "alice");
        let json = serde_json::to_string(&g).unwrap();
        let decoded: Group = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, "DevTeam");
        assert_eq!(decoded.owner, "alice");
        assert_eq!(decoded.members, vec!["alice"]);
    }

    // ── GroupAction variants ────────────────────────────────────────────────

    #[test]
    fn action_create_round_trip() {
        let g = make_group("Team", "alice");
        let action = GroupAction::Create { group: g.clone() };
        let event = GroupEvent { action };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: GroupEvent = serde_json::from_str(&json).unwrap();
        match decoded.action {
            GroupAction::Create { group } => assert_eq!(group.name, "Team"),
            _ => panic!("Mauvais variant"),
        }
    }

    #[test]
    fn action_add_member_round_trip() {
        let event = GroupEvent {
            action: GroupAction::AddMember {
                group_name: "Team".to_string(),
                username: "bob".to_string(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: GroupEvent = serde_json::from_str(&json).unwrap();
        match decoded.action {
            GroupAction::AddMember { group_name, username } => {
                assert_eq!(group_name, "Team");
                assert_eq!(username, "bob");
            }
            _ => panic!("Mauvais variant"),
        }
    }

    #[test]
    fn action_remove_member_round_trip() {
        let event = GroupEvent {
            action: GroupAction::RemoveMember {
                group_name: "Team".to_string(),
                username: "charlie".to_string(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: GroupEvent = serde_json::from_str(&json).unwrap();
        match decoded.action {
            GroupAction::RemoveMember { group_name, username } => {
                assert_eq!(group_name, "Team");
                assert_eq!(username, "charlie");
            }
            _ => panic!("Mauvais variant"),
        }
    }

    #[test]
    fn action_rename_round_trip() {
        let event = GroupEvent {
            action: GroupAction::Rename {
                group_name: "OldName".to_string(),
                new_name: "NewName".to_string(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: GroupEvent = serde_json::from_str(&json).unwrap();
        match decoded.action {
            GroupAction::Rename { group_name, new_name } => {
                assert_eq!(group_name, "OldName");
                assert_eq!(new_name, "NewName");
            }
            _ => panic!("Mauvais variant"),
        }
    }

    #[test]
    fn action_delete_round_trip() {
        let event = GroupEvent {
            action: GroupAction::Delete { group_name: "OldTeam".to_string() },
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: GroupEvent = serde_json::from_str(&json).unwrap();
        match decoded.action {
            GroupAction::Delete { group_name } => assert_eq!(group_name, "OldTeam"),
            _ => panic!("Mauvais variant"),
        }
    }

    #[test]
    fn group_members_preserved() {
        let mut g = make_group("Team", "alice");
        g.members.push("bob".to_string());
        g.members.push("charlie".to_string());
        let json = serde_json::to_string(&g).unwrap();
        let decoded: Group = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.members.len(), 3);
        assert!(decoded.members.contains(&"bob".to_string()));
    }
}
