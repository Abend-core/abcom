
use super::{day_divider_label, starts_new_group, GROUP_BREAK_SECS};
use crate::ui::UiLanguage;
use chrono::NaiveDate;

#[test]
fn group_breaks_on_author_change() {
    assert!(starts_new_group(
        Some("alice"),
        Some(100),
        "bob",
        Some(110),
        false
    ));
}

#[test]
fn group_breaks_on_day_change() {
    assert!(starts_new_group(
        Some("alice"),
        Some(100),
        "alice",
        Some(110),
        true
    ));
}

#[test]
fn group_keeps_same_author_within_window() {
    assert!(!starts_new_group(
        Some("alice"),
        Some(1_000),
        "alice",
        Some(1_000 + GROUP_BREAK_SECS),
        false,
    ));
}

#[test]
fn group_breaks_after_time_gap() {
    assert!(starts_new_group(
        Some("alice"),
        Some(1_000),
        "alice",
        Some(1_000 + GROUP_BREAK_SECS + 1),
        false,
    ));
}

#[test]
fn group_falls_back_to_author_without_epoch() {
    // Sans instants comparables : même auteur reste groupé.
    assert!(!starts_new_group(Some("alice"), None, "alice", None, false));
}

#[test]
fn divider_labels_today_and_yesterday() {
    let today = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
    let yesterday = NaiveDate::from_ymd_opt(2026, 5, 19).unwrap();
    assert_eq!(
        day_divider_label(today, today, UiLanguage::French),
        "Aujourd'hui"
    );
    assert_eq!(
        day_divider_label(today, today, UiLanguage::English),
        "Today"
    );
    assert_eq!(
        day_divider_label(yesterday, today, UiLanguage::French),
        "Hier"
    );
    assert_eq!(
        day_divider_label(yesterday, today, UiLanguage::English),
        "Yesterday"
    );
}

#[test]
fn divider_labels_full_date_localized() {
    let today = NaiveDate::from_ymd_opt(2026, 6, 23).unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 5, 18).unwrap();
    assert_eq!(
        day_divider_label(date, today, UiLanguage::French),
        "18 mai 2026"
    );
    assert_eq!(
        day_divider_label(date, today, UiLanguage::English),
        "May 18, 2026"
    );
}
