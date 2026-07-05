use std::path::PathBuf;

use super::{attachment_label, push_unique_paths, should_send_message};

#[test]
fn enter_from_composer_sends_when_shortcode_menu_is_closed() {
    assert!(should_send_message(true, false, false, "hello"));
}

#[test]
fn enter_fallback_does_not_send_when_shortcode_menu_is_open() {
    assert!(!should_send_message(false, true, true, ":jo"));
}

#[test]
fn enter_fallback_sends_when_shortcode_menu_is_closed() {
    assert!(should_send_message(false, true, false, "hello"));
}

#[test]
fn empty_message_never_sends() {
    assert!(!should_send_message(true, true, false, "   "));
}

#[test]
fn push_unique_paths_ignores_duplicates() {
    let mut paths = vec![PathBuf::from("/tmp/alpha.txt")];

    push_unique_paths(
        &mut paths,
        [
            PathBuf::from("/tmp/alpha.txt"),
            PathBuf::from("/tmp/beta.txt"),
            PathBuf::from("/tmp/beta.txt"),
        ],
    );

    assert_eq!(
        paths,
        vec![
            PathBuf::from("/tmp/alpha.txt"),
            PathBuf::from("/tmp/beta.txt")
        ]
    );
}

#[test]
fn attachment_label_prefers_file_name() {
    assert_eq!(
        attachment_label(PathBuf::from("/tmp/subdir/report.pdf").as_path()),
        "report.pdf"
    );
}

#[test]
fn attachment_label_falls_back_to_full_path_when_needed() {
    let path = PathBuf::from("/");
    assert_eq!(attachment_label(path.as_path()), "/");
}
