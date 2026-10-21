mod common;

use std::time::Duration;

use common::{post, start};
use mx_claude::{ChipLevel, ContextUsage, ServerEvent, format_tokens};
use serde_json::{Value, json};

fn status(used: Value, usage: Option<Value>, size: Option<i64>) -> Value {
    let mut window = serde_json::Map::new();
    window.insert("used_percentage".into(), used);
    if let Some(size) = size {
        window.insert("context_window_size".into(), json!(size));
    }
    window.insert(
        "current_usage".into(),
        usage.unwrap_or(json!({
            "input_tokens": 10,
            "cache_creation_input_tokens": 1000,
            "cache_read_input_tokens": 115000,
            "output_tokens": 300
        })),
    );
    json!({ "context_window": window })
}

fn valid() -> Value {
    status(json!(58.4), None, Some(200000))
}

#[test]
fn reads_rounded_percent_tokens_without_output_and_window() {
    let c = ContextUsage::from_status_line(&valid()).unwrap();
    assert_eq!((c.percent, c.tokens, c.window), (58, 116010, 200000));
}

#[test]
fn missing_numbers_mean_no_reading() {
    let none = |v: Value| assert_eq!(ContextUsage::from_status_line(&v), None, "{v}");
    none(status(Value::Null, None, Some(200000)));
    none(status(json!(58.4), Some(Value::Null), Some(200000)));
    none(json!({"model": "x"}));
    none(json!("nada"));
    none(status(json!(58.4), None, None));
    none(status(json!(58.4), None, Some(0)));
    none(
        json!({"context_window": {"used_percentage": "x", "context_window_size": 200000, "current_usage": {"input_tokens": 1}}}),
    );
    none(
        json!({"context_window": {"used_percentage": 50, "context_window_size": 200000, "current_usage": "x"}}),
    );
    none(json!({"context_window": "x"}));
}

#[tokio::test]
async fn status_goes_to_statuses_and_hooks_stay_hooks() {
    let state = tempfile::tempdir().unwrap();
    let (server, mut rx) = start(state.path()).await;
    post(server.port(), "/hook/tab9", r#"{"hook_event_name":"Stop"}"#).await;
    post(server.port(), "/status/tab9", &valid().to_string()).await;

    let mut events = Vec::new();
    for _ in 0..2 {
        let e = tokio::time::timeout(Duration::from_secs(5), rx.recv());
        events.push(e.await.unwrap().unwrap());
    }
    let hooks: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            ServerEvent::Hook { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(hooks, ["Stop"]);
    let status = events
        .iter()
        .find_map(|e| match e {
            ServerEvent::Status { tab_id, payload } => Some((tab_id, payload)),
            _ => None,
        })
        .unwrap();
    assert_eq!(status.0, "tab9");
    assert!(status.1["context_window"].is_object());
}

#[test]
fn format_tokens_switches_units_without_reaching_a_thousand_thousands() {
    let cases = [
        (999, "999"),
        (1000, "1 mil"),
        (1499, "1 mil"),
        (1500, "2 mil"),
        (116010, "116 mil"),
        (116900, "117 mil"),
        (999499, "999 mil"),
        (999500, "1 mi"),
        (1000000, "1 mi"),
        (1040000, "1 mi"),
        (1050000, "1,1 mi"),
        (1200000, "1,2 mi"),
        (1950000, "2 mi"),
        (1999999, "2 mi"),
        (200000, "200 mil"),
    ];
    for (n, text) in cases {
        assert_eq!(format_tokens(n), text, "{n}");
    }
}

#[test]
fn chip_fills_one_segment_per_twenty_percent_and_warms_up() {
    let chip = |percent| ContextUsage {
        percent,
        tokens: 116010,
        window: 200000,
    };
    let filled: Vec<usize> = [0, 1, 20, 21, 100].map(|p| chip(p).chip_filled()).to_vec();
    assert_eq!(filled, [0, 1, 1, 2, 5]);
    assert_eq!(chip(59).chip_level(), ChipLevel::Calm);
    assert_eq!(chip(60).chip_level(), ChipLevel::Warn);
    assert_eq!(chip(84).chip_level(), ChipLevel::Warn);
    assert_eq!(chip(85).chip_level(), ChipLevel::Danger);
    assert_eq!(chip(58).chip_tooltip(), "116 mil de 200 mil tokens");
    assert_eq!(chip(58).chip_text(), "58%");
}
