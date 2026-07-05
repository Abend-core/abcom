use super::*;

#[test]
fn trending_url_contains_key_and_pagination() {
    let url = trending_url(ContentKind::Gif, "MYKEY", "fr", 2);
    assert!(url.contains("/MYKEY/gifs/trending"));
    assert!(url.contains("page=2"));
    assert!(url.contains(&format!("per_page={PER_PAGE}")));
    assert!(url.contains("locale=fr"));
}

#[test]
fn meme_trending_url_uses_static_memes_segment() {
    let url = trending_url(ContentKind::Meme, "K", "fr", 1);
    assert!(url.contains("/K/static-memes/trending"));
}

#[test]
fn sticker_search_url_uses_stickers_segment() {
    let url = search_url(ContentKind::Sticker, "K", "en", "cute", 1);
    assert!(url.contains("/K/stickers/search"));
    assert!(url.contains("q=cute"));
}

#[test]
fn search_url_encodes_query() {
    let url = search_url(ContentKind::Gif, "K", "en", "happy cat", 1);
    assert!(url.contains("/K/gifs/search"));
    assert!(url.contains("q=happy%20cat"));
    assert!(url.contains("page=1"));
}

#[test]
fn parse_extracts_xs_and_hd_webp() {
    let body = br#"{
            "result": true,
            "data": {
                "data": [
                    {
                        "id": 123,
                        "file": {
                            "xs": {"webp": {"url": "https://x/xs.webp", "width": 90, "height": 60}},
                            "hd": {"webp": {"url": "https://x/hd.webp", "width": 480, "height": 320}}
                        }
                    }
                ],
                "current_page": 1,
                "per_page": 24,
                "has_next": true
            }
        }"#;
    let (items, has_next) = parse(body).unwrap();
    assert!(has_next);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "123");
    assert_eq!(items[0].preview_url, "https://x/xs.webp");
    assert_eq!(items[0].full_url, "https://x/hd.webp");
    assert_eq!(items[0].width, Some(480));
    assert_eq!(items[0].height, Some(320));
}

#[test]
fn parse_falls_back_to_gif_when_no_webp() {
    let body = br#"{"data":{"data":[{"id":"abc","file":{"sm":{"gif":{"url":"https://x/sm.gif"}}}}],"has_next":false}}"#;
    let (items, _) = parse(body).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].preview_url, "https://x/sm.gif");
}

#[test]
fn parse_skips_items_without_files() {
    let body = br#"{"data":{"data":[{"id":"empty","file":{}}],"has_next":false}}"#;
    let (items, _) = parse(body).unwrap();
    assert!(items.is_empty());
}
