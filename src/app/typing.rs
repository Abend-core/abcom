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
