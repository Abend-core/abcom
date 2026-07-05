use crate::app::AppState;

#[test]
fn media_path_under_data_dir() {
    let dir = std::env::temp_dir().join(format!("abcom_media_{}", std::process::id()));
    let s = AppState::new_with_base("alice", &dir);
    assert_eq!(s.media_path("x.bin"), dir.join("media").join("x.bin"));
    assert!(s.media_bytes("absent.bin").is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn media_bytes_reads_written_file() {
    let dir = std::env::temp_dir().join(format!("abcom_media2_{}", std::process::id()));
    let s = AppState::new_with_base("alice", &dir);
    std::fs::create_dir_all(dir.join("media")).unwrap();
    std::fs::write(s.media_path("y.bin"), [7, 8, 9]).unwrap();
    assert_eq!(s.media_bytes("y.bin"), Some(vec![7, 8, 9]));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn remove_media_message_drops_message_and_file() {
    use crate::message::{ChatMessage, MediaAttachment, MediaKind};
    let dir = std::env::temp_dir().join(format!("abcom_media3_{}", std::process::id()));
    let mut s = AppState::new_with_base("alice", &dir);
    std::fs::create_dir_all(dir.join("media")).unwrap();
    std::fs::write(s.media_path("z.bin"), [1, 2, 3]).unwrap();
    s.messages.push(ChatMessage {
        from: "bob".to_string(),
        content: String::new(),
        timestamp: "12:00".to_string(),
        timestamp_epoch: None,
        to_user: Some("alice".to_string()),
        media: Some(MediaAttachment {
            id: "z.bin".to_string(),
            filename: "z.bin".to_string(),
            kind: MediaKind::File,
            size_bytes: 3,
            url: None,
            width: None,
            height: None,
        }),
        reply_to: None,
        nonce: None,
    });

    s.remove_media_message("z.bin");

    assert!(s.messages.is_empty(), "le message média doit être retiré");
    assert!(
        s.media_bytes("z.bin").is_none(),
        "le fichier doit être supprimé"
    );
    std::fs::remove_dir_all(&dir).ok();
}
