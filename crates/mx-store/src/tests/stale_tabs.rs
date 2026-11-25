use mx_claude::mcp::NewSuggestion;
use serde_json::json;

use super::Rig;

fn fixture() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("crc");
    rig.s.tab_mut("crc").unwrap().session_id = Some("sid-crc".into());
    rig
}

#[test]
fn a_hook_from_a_previous_run_lands_on_the_tab_holding_its_session() {
    let mut rig = fixture();
    rig.hook(
        "tabold_18",
        "UserPromptSubmit",
        json!({"session_id": "sid-crc"}),
    );
    assert!(rig.t("crc").hooks.status.is_busy());
    rig.hook("tabold_18", "Stop", json!({}));
    assert!(!rig.t("crc").hooks.status.is_busy());
}

#[test]
fn mcp_calls_from_a_previous_run_reach_the_same_tab_once_it_is_known() {
    let mut rig = fixture();
    rig.hook("tabold_18", "PreToolUse", json!({"session_id": "sid-crc"}));
    let q = rig.s.suggest(NewSuggestion {
        tab_id: "tabold_18".into(),
        title: "x".into(),
        tldr: "y".into(),
        prompt: "z".into(),
    });
    assert!(q.is_some());
    assert_eq!(rig.t("crc").hooks.suggestions.len(), 1);
}

#[test]
fn a_hook_for_an_unknown_tab_and_session_is_dropped() {
    let mut rig = fixture();
    rig.hook(
        "tabold_18",
        "UserPromptSubmit",
        json!({"session_id": "outra"}),
    );
    rig.hook("tabold_18", "UserPromptSubmit", json!({}));
    assert!(!rig.t("crc").hooks.status.is_busy());
}
