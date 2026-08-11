use super::SidebarCache;
use crate::app::AppState;
use std::net::SocketAddr;

fn state() -> AppState {
    let dir = std::env::temp_dir().join(format!(
        "abcom_sidebar_cache_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    AppState::new_with_base("alice", &dir)
}

fn addr(ip: &str) -> SocketAddr {
    format!("{}:9000", ip).parse().unwrap()
}

#[test]
fn pinned_peer_sorts_before_unpinned() {
    let mut s = state();
    s.add_peer("bob".to_string(), addr("192.168.1.5"));
    s.add_peer("carol".to_string(), addr("192.168.1.6"));
    s.toggle_pinned("carol");

    let mut cache = SidebarCache::default();
    cache.refresh(&s);

    assert_eq!(cache.peers[0].username, "carol");
    assert_eq!(cache.peers[1].username, "bob");
    assert!(cache.peer_pinned[0]);
    assert!(!cache.peer_pinned[1]);
}

#[test]
fn pinned_group_sorts_before_unpinned() {
    let mut s = state();
    s.create_group("alpha".to_string(), vec![]);
    let beta = s
        .create_group("beta".to_string(), vec![])
        .expect("création du salon");
    s.toggle_pinned(&AppState::group_conv_key(&beta.id));

    let mut cache = SidebarCache::default();
    cache.refresh(&s);

    assert_eq!(cache.groups[0].name, "beta");
    assert_eq!(cache.groups[1].name, "alpha");
    assert!(cache.group_pinned[0]);
    assert!(!cache.group_pinned[1]);
}

// ── Repli des messages très longs ─────────────────────────────────────────

#[test]
fn short_message_is_not_collapsed() {
    let emoji_map = std::collections::HashMap::new();
    assert!(super::collapse_info("un message normal", &emoji_map).is_none());
}

#[test]
fn very_long_single_line_is_collapsed_with_char_preview() {
    let emoji_map = std::collections::HashMap::new();
    let content = "mot ".repeat(2_000); // 8 000 caractères, une seule ligne
    let info = super::collapse_info(&content, &emoji_map).expect("doit être replié");
    assert_eq!(info.total_chars, 8_000);
    assert_eq!(info.total_lines, 1);
}

#[test]
fn many_lines_are_collapsed_even_if_short_in_chars() {
    let emoji_map = std::collections::HashMap::new();
    let content = "ligne\n".repeat(100); // 100 lignes, ~600 caractères
    let info = super::collapse_info(&content, &emoji_map).expect("doit être replié");
    assert_eq!(info.total_lines, 100);
}

#[test]
fn collapse_preview_counts_unicode_chars_not_bytes() {
    let emoji_map = std::collections::HashMap::new();
    // Contenu 100 % multi-octets : la coupe de l'aperçu ne doit pas
    // paniquer sur une frontière UTF-8.
    let content = "héhé😀".repeat(1_000); // 5 000 caractères
    let info = super::collapse_info(&content, &emoji_map).expect("doit être replié");
    assert_eq!(info.total_chars, 5_000);
}
