use super::{extension_lower, MediaAttachment, MediaKind};

#[test]
fn detects_image_extensions() {
    assert!(MediaAttachment::is_image_filename("photo.PNG"));
    assert!(MediaAttachment::is_image_filename("a.jpeg"));
    assert!(!MediaAttachment::is_image_filename("rapport.pdf"));
    assert!(!MediaAttachment::is_image_filename("sans_extension"));
}

#[test]
fn extension_is_lowercased() {
    assert_eq!(extension_lower("Image.JpG").as_deref(), Some("jpg"));
    assert_eq!(extension_lower("archive.tar.gz").as_deref(), Some("gz"));
    assert_eq!(extension_lower("noext"), None);
}

#[test]
fn media_round_trip() {
    let att = MediaAttachment {
        id: "abc.png".to_string(),
        filename: "abc.png".to_string(),
        kind: MediaKind::Image,
        size_bytes: 3,
        url: None,
        width: Some(10),
        height: Some(20),
    };
    let json = serde_json::to_string(&att).unwrap();
    let back: MediaAttachment = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, MediaKind::Image);
    assert_eq!(back.filename, "abc.png");
    assert_eq!(back.width, Some(10));
}

#[test]
fn media_omits_absent_dimensions() {
    let att = MediaAttachment {
        id: "f.bin".to_string(),
        filename: "f.bin".to_string(),
        kind: MediaKind::File,
        size_bytes: 0,
        url: None,
        width: None,
        height: None,
    };
    let json = serde_json::to_string(&att).unwrap();
    assert!(!json.contains("width"));
}

#[test]
fn gif_media_round_trip_keeps_url() {
    let att = MediaAttachment {
        id: "klipy-42".to_string(),
        filename: "gif.webp".to_string(),
        kind: MediaKind::Gif,
        size_bytes: 0,
        url: Some("https://cdn.klipy.com/hd.webp".to_string()),
        width: Some(480),
        height: Some(320),
    };
    let json = serde_json::to_string(&att).unwrap();
    assert!(json.contains("\"kind\":\"gif\""));
    let back: MediaAttachment = serde_json::from_str(&json).unwrap();
    assert_eq!(back.kind, MediaKind::Gif);
    assert_eq!(back.url.as_deref(), Some("https://cdn.klipy.com/hd.webp"));
}

#[test]
fn stream_header_round_trip() {
    let header = super::MediaStreamHeader {
        from: "bob".to_string(),
        to_user: Some("ellis".to_string()),
        timestamp: "12:00".to_string(),
        timestamp_epoch: Some(1_750_000_000),
        media: MediaAttachment {
            id: "x.zip".to_string(),
            filename: "x.zip".to_string(),
            kind: MediaKind::File,
            size_bytes: 6_000_000_000,
            url: None,
            width: None,
            height: None,
        },
        requires_ack: true,
    };
    let json = serde_json::to_string(&header).unwrap();
    let back: super::MediaStreamHeader = serde_json::from_str(&json).unwrap();
    assert_eq!(back.media.size_bytes, 6_000_000_000);
    assert!(back.requires_ack);
    assert_eq!(back.to_user.as_deref(), Some("ellis"));
}
