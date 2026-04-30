use std::time::SystemTime;
use super::AppState;

impl AppState {
    pub fn set_user_typing(&mut self, username: String) {
        self.typing_users.insert(username, SystemTime::now());
    }

    pub fn clear_typing_if_old(&mut self) {
        let now = SystemTime::now();
        self.typing_users.retain(|_, time| {
            now.duration_since(*time)
                .map(|d| d.as_secs() < 3)
                .unwrap_or(false)
        });
    }

    pub fn typing_users_list(&self) -> Vec<String> {
        self.typing_users.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, Duration};
    use crate::app::AppState;

    fn state() -> AppState {
        let mut s = AppState::new("alice".to_string());
        s.typing_users.clear();
        s
    }

    #[test]
    fn test_set_and_list_typing() {
        let mut s = state();
        s.set_user_typing("bob".to_string());
        s.set_user_typing("charlie".to_string());
        let list = s.typing_users_list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"bob".to_string()));
        assert!(list.contains(&"charlie".to_string()));
    }

    #[test]
    fn test_clear_typing_keeps_recent() {
        let mut s = state();
        s.set_user_typing("bob".to_string()); // now
        s.clear_typing_if_old();
        assert_eq!(s.typing_users_list().len(), 1);
    }

    #[test]
    fn test_clear_typing_removes_old() {
        let mut s = state();
        // Inject a timestamp 5 seconds in the past
        let old_time = SystemTime::now() - Duration::from_secs(5);
        s.typing_users.insert("bob".to_string(), old_time);
        s.clear_typing_if_old();
        assert!(s.typing_users_list().is_empty());
    }

    #[test]
    fn test_clear_typing_mixed() {
        let mut s = state();
        let old_time = SystemTime::now() - Duration::from_secs(10);
        s.typing_users.insert("old".to_string(), old_time);
        s.set_user_typing("recent".to_string());
        s.clear_typing_if_old();
        let list = s.typing_users_list();
        assert_eq!(list.len(), 1);
        assert!(list.contains(&"recent".to_string()));
    }
}
