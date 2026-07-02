
use super::update_recent_emojis;

#[test]
fn moves_existing_to_front() {
    let mut recent = vec!["👍".to_string(), "❤️".to_string(), "😂".to_string()];
    update_recent_emojis(&mut recent, "😂", 6);
    assert_eq!(recent, vec!["😂", "👍", "❤️"]);
}

#[test]
fn prepends_new() {
    let mut recent = vec!["👍".to_string()];
    update_recent_emojis(&mut recent, "🔥", 6);
    assert_eq!(recent, vec!["🔥", "👍"]);
}

#[test]
fn truncates_at_max_len() {
    let mut recent = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    update_recent_emojis(&mut recent, "d", 3);
    assert_eq!(recent, vec!["d", "a", "b"]);
}
