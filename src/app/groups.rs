use crate::message::Group;
use super::AppState;

impl AppState {
    fn validate_group_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 50
            && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    }

    pub fn create_group(&mut self, name: String, members: Vec<String>) -> Option<Group> {
        let name = name.trim().to_string();
        if !Self::validate_group_name(&name) {
            return None;
        }
        if self.groups.iter().any(|g| g.name.eq_ignore_ascii_case(&name)) {
            return None;
        }
        let invalid: Vec<_> = members.iter()
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
        if let Some(g) = self.groups.iter_mut().find(|g| g.name == group_name) {
            if g.owner == self.my_username && !g.members.contains(&username) {
                g.members.push(username);
                self.save_groups();
                return true;
            }
        }
        false
    }

    pub fn remove_member_from_group(&mut self, group_name: &str, username: &str) -> bool {
        if let Some(g) = self.groups.iter_mut().find(|g| g.name == group_name) {
            if g.owner == self.my_username && username != &g.owner {
                g.members.retain(|m| m != username);
                self.save_groups();
                return true;
            }
        }
        false
    }

    pub fn rename_group(&mut self, group_name: &str, new_name: String) -> bool {
        if let Some(g) = self.groups.iter_mut().find(|g| g.name == group_name) {
            if g.owner == self.my_username {
                g.name = new_name;
                self.save_groups();
                return true;
            }
        }
        false
    }

    pub fn delete_group(&mut self, group_name: &str) -> bool {
        if let Some(pos) = self.groups.iter().position(|g| g.name == group_name && g.owner == self.my_username) {
            self.groups.remove(pos);
            self.save_groups();
            return true;
        }
        false
    }

    pub fn get_group(&self, group_name: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.name == group_name)
    }

    pub fn is_group_owner(&self, group_name: &str) -> bool {
        self.groups.iter().any(|g| g.name == group_name && g.owner == self.my_username)
    }

    pub fn is_in_group(&self, group_name: &str) -> bool {
        self.groups.iter().any(|g| g.name == group_name && g.members.contains(&self.my_username))
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{AppState, Peer};

    fn new_test_state(username: &str) -> AppState {
        let mut s = AppState::new(username.to_string());
        s.groups.clear();
        s.messages.clear();
        s.peers.clear();
        s.read_counts.clear();
        s
    }

    #[test]
    fn test_validate_group_name_valid() {
        assert!(AppState::validate_group_name("my-group"));
        assert!(AppState::validate_group_name("group_123"));
        assert!(AppState::validate_group_name("DevTeam"));
    }

    #[test]
    fn test_validate_group_name_invalid() {
        assert!(!AppState::validate_group_name(""));
        assert!(!AppState::validate_group_name(&"x".repeat(51)));
        assert!(!AppState::validate_group_name("group@name"));
        assert!(!AppState::validate_group_name("group name"));
    }

    #[test]
    fn test_create_group_success() {
        let mut s = new_test_state("alice");
        s.peers.push(Peer { username: "bob".into(), addr: "127.0.0.1:9000".parse().unwrap(), last_seen: 0, online: true });
        let g = s.create_group("DevTeam".into(), vec!["bob".into()]);
        assert!(g.is_some());
        assert_eq!(s.groups[0].members.len(), 2);
    }

    #[test]
    fn test_create_group_invalid_name() {
        let mut s = new_test_state("alice");
        assert!(s.create_group("".into(), vec![]).is_none());
    }

    #[test]
    fn test_create_group_duplicate() {
        let mut s = new_test_state("alice");
        s.create_group("DevTeam".into(), vec![]);
        assert!(s.create_group("DevTeam".into(), vec![]).is_none());
        assert_eq!(s.groups.len(), 1);
    }

    #[test]
    fn test_create_group_invalid_member() {
        let mut s = new_test_state("alice");
        assert!(s.create_group("Team".into(), vec!["unknown".into()]).is_none());
    }

    #[test]
    fn test_is_group_owner() {
        let mut s = new_test_state("alice");
        s.create_group("MyGroup".into(), vec![]);
        assert!(s.is_group_owner("MyGroup"));
        assert!(!s.is_group_owner("NonExistent"));
    }

    #[test]
    fn test_add_remove_member() {
        let mut s = new_test_state("alice");
        s.peers.push(Peer { username: "bob".into(), addr: "127.0.0.1:9000".parse().unwrap(), last_seen: 0, online: true });
        s.create_group("Team".into(), vec![]);
        assert!(s.add_member_to_group("Team", "bob".into()));
        assert_eq!(s.groups[0].members.len(), 2);
        assert!(s.remove_member_from_group("Team", "bob"));
        assert_eq!(s.groups[0].members.len(), 1);
    }

    #[test]
    fn test_get_online_peers() {
        let mut s = new_test_state("alice");
        s.peers.push(Peer { username: "bob".into(), addr: "192.168.1.10:9000".parse().unwrap(), last_seen: 0, online: true });
        s.peers.push(Peer { username: "charlie".into(), addr: "192.168.1.11:9000".parse().unwrap(), last_seen: 0, online: false });
        let online = s.get_online_peers();
        assert_eq!(online.len(), 1);
        assert!(online.contains(&"192.168.1.10:9000".parse().unwrap()));
    }
}
