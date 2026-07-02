
use super::AvatarAnnounce;

#[test]
fn avatar_announce_round_trip() {
    let a = AvatarAnnounce {
        from: "alice".to_string(),
        png: vec![1, 2, 3, 4],
    };
    let json = serde_json::to_string(&a).unwrap();
    let decoded: AvatarAnnounce = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.from, "alice");
    assert_eq!(decoded.png, vec![1, 2, 3, 4]);
}

#[test]
fn avatar_announce_empty_marks_removal() {
    let a = AvatarAnnounce {
        from: "bob".to_string(),
        png: Vec::new(),
    };
    let json = serde_json::to_string(&a).unwrap();
    let decoded: AvatarAnnounce = serde_json::from_str(&json).unwrap();
    assert!(decoded.png.is_empty());
}
