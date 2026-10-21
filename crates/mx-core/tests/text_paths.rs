use std::path::PathBuf;

use chrono::{Duration, Local, TimeZone, Timelike};
use mx_core::paths::{expand_home_with, state_dir_from};
use mx_core::text::{clip_or_ellipsis, iso8601, parse_iso8601, short_ago, short_until};
use proptest::prelude::*;

const HOME: Option<&str> = Some("/home/u");

#[test]
fn short_ago_buckets_by_unit() {
    let at = Local.with_ymd_and_hms(2026, 9, 9, 12, 0, 0).unwrap();
    let ago = |d: Duration| short_ago(&at, &(at + d));
    assert_eq!(ago(Duration::seconds(3)), "agora");
    assert_eq!(ago(Duration::seconds(59)), "agora");
    assert_eq!(ago(Duration::minutes(1)), "1min");
    assert_eq!(ago(Duration::minutes(59)), "59min");
    assert_eq!(ago(Duration::hours(3)), "3h");
    assert_eq!(ago(Duration::days(2)), "2d");
    assert_eq!(ago(Duration::days(9)), "9/9");
}

#[test]
fn short_until_buckets_by_unit() {
    let now = Local.with_ymd_and_hms(2026, 9, 9, 12, 0, 0).unwrap();
    let until = |d: Duration| short_until(&(now + d), &now);
    assert_eq!(until(Duration::seconds(-1)), "agora");
    assert_eq!(until(Duration::seconds(30)), "<1min");
    assert_eq!(until(Duration::minutes(5)), "5min");
    assert_eq!(until(Duration::hours(2)), "2h");
    assert_eq!(until(Duration::days(3)), "3d");
    assert_eq!(until(Duration::days(30)), "9/10");
}

#[test]
fn iso8601_matches_dart_local_format() {
    let at = Local.with_ymd_and_hms(2026, 9, 18, 7, 5, 3).unwrap();
    assert_eq!(iso8601(&at), "2026-09-18T07:05:03.000");
    let fine = at.with_nanosecond(123_456_000).unwrap();
    assert_eq!(iso8601(&fine), "2026-09-18T07:05:03.123456");
    let ms = at.with_nanosecond(120_000_000).unwrap();
    assert_eq!(iso8601(&ms), "2026-09-18T07:05:03.120");
}

#[test]
fn iso8601_parses_leniently() {
    let at = Local.with_ymd_and_hms(2026, 9, 18, 7, 5, 3).unwrap();
    assert_eq!(parse_iso8601("2026-09-18T07:05:03"), Some(at));
    assert_eq!(parse_iso8601("2026-09-18T07:05:03.000"), Some(at));
    let utc = parse_iso8601("2026-09-18T10:05:03Z").unwrap();
    assert_eq!(
        utc.timestamp(),
        chrono::Utc
            .with_ymd_and_hms(2026, 9, 18, 10, 5, 3)
            .unwrap()
            .timestamp()
    );
    let offset = parse_iso8601("2026-09-18T07:05:03.5-03:00").unwrap();
    assert_eq!(offset.timestamp(), utc.timestamp());
    assert_eq!(parse_iso8601("ontem"), None);
    assert_eq!(parse_iso8601(""), None);
}

#[test]
fn clip_appends_ellipsis_only_past_the_limit() {
    assert_eq!(clip_or_ellipsis("abc", 3), "abc");
    assert_eq!(clip_or_ellipsis("abcd", 3), "abc…");
    assert_eq!(clip_or_ellipsis("a😀b", 2), "a…");
}

#[test]
fn tilde_paths_expand_to_home() {
    assert_eq!(expand_home_with("~/repos/foo", HOME), "/home/u/repos/foo");
    assert_eq!(expand_home_with("~/", HOME), "/home/u/");
    assert_eq!(expand_home_with("~", HOME), "/home/u");
    assert_eq!(expand_home_with("~/x", Some("/home/u/")), "/home/u/x");
}

#[test]
fn other_users_tilde_is_left_alone() {
    assert_eq!(expand_home_with("~outro/repos", HOME), "~outro/repos");
    assert_eq!(expand_home_with("~outro", HOME), "~outro");
}

#[test]
fn absolute_relative_empty_and_trailing_tilde_are_unchanged() {
    for p in [
        "/Volumes/Externo/repos/foo",
        "repos/foo",
        "./foo",
        "",
        "/tmp/notas.md~",
    ] {
        assert_eq!(expand_home_with(p, HOME), p);
    }
    assert_eq!(expand_home_with("~/x", None), "~/x");
    assert_eq!(expand_home_with("~/x", Some("")), "~/x");
}

#[test]
fn state_dir_honours_the_override_and_never_defaults_elsewhere() {
    assert_eq!(
        state_dir_from(Some("/tmp/mx-test"), HOME),
        PathBuf::from("/tmp/mx-test")
    );
    assert_eq!(
        state_dir_from(Some("~/alt"), HOME),
        PathBuf::from("/home/u/alt")
    );
    assert_eq!(
        state_dir_from(Some("  "), HOME),
        PathBuf::from("/home/u/.maestria")
    );
    assert_eq!(
        state_dir_from(None, HOME),
        PathBuf::from("/home/u/.maestria")
    );
    assert_eq!(state_dir_from(None, None), PathBuf::from("/.maestria"));
}

proptest! {
    #[test]
    fn utf16_clip_never_splits_a_character_or_exceeds_the_limit(s in "\\PC{0,40}", max in 0usize..50) {
        let clipped = mx_core::text::clip_utf16(&s, max);
        prop_assert!(mx_core::text::utf16_len(clipped) <= max);
        prop_assert!(s.starts_with(clipped));
    }

    #[test]
    fn iso8601_round_trips_to_the_microsecond(secs in 0i64..4_000_000_000, micros in 0u32..1_000_000) {
        let at = Local.timestamp_opt(secs, micros * 1000).single();
        prop_assume!(at.is_some());
        let at = at.unwrap();
        let back = parse_iso8601(&iso8601(&at));
        prop_assume!(back.is_some_and(|b| b.offset() == at.offset()));
        prop_assert_eq!(back.unwrap(), at);
    }
}
