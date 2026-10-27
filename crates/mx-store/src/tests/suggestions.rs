use mx_claude::mcp::{NewSuggestion, SuggestionEnd as WireEnd, is_task_id};
use mx_core::model::{NoticeKind, SuggestionEnd};
use serde_json::json;

use super::Rig;

fn fixture() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("implementa");
    rig
}

fn ask(rig: &mut Rig, title: &str) -> String {
    let q = rig.s.suggest(NewSuggestion {
        tab_id: "implementa".into(),
        title: title.into(),
        tldr: "curto".into(),
        prompt: format!("faz {title}"),
    });
    q.unwrap().task.id
}

#[test]
fn suggest_queues_a_task_and_a_notice() {
    let mut rig = fixture();
    let id = ask(&mut rig, "Converter SQL");
    assert!(is_task_id(&id));
    assert_eq!(rig.t("implementa").hooks.suggestions.len(), 1);
    assert_eq!(rig.s.notices()[0].notice.kind, NoticeKind::Suggested);
    assert_eq!(rig.s.unread_notices(), 1);
    assert_eq!(rig.t("implementa").to_json()["suggestions"].as_array().unwrap().len(), 1);
}

#[test]
fn working_does_not_read_the_suggestion_notice() {
    let mut rig = fixture();
    rig.hook("implementa", "UserPromptSubmit", json!({}));
    rig.hook("implementa", "PreToolUse", json!({"tool_name": "mcp__maestria__suggest_task"}));
    ask(&mut rig, "x");
    rig.hook("implementa", "PostToolUse", json!({"tool_name": "mcp__maestria__suggest_task"}));
    assert!(!rig.s.notices()[0].notice.read);
}

#[test]
fn withdraw_twice_reports_the_settlement() {
    let mut rig = fixture();
    let id = ask(&mut rig, "x");
    assert!(rig.s.withdraw("implementa", &id).withdrawn);
    let again = rig.s.withdraw("implementa", &id);
    assert!(!again.withdrawn);
    assert_eq!(again.already, Some(WireEnd::Dismissed));
}

#[test]
fn unknown_or_non_claude_tabs_refuse() {
    let mut rig = fixture();
    rig.shell("s");
    for tab in ["ghost", "s"] {
        let q = rig.s.suggest(NewSuggestion { tab_id: tab.into(), title: "".into(), tldr: "".into(), prompt: "p".into() });
        assert!(q.is_none());
    }
}

#[test]
fn the_twenty_first_evicts_the_oldest() {
    let mut rig = fixture();
    let first = ask(&mut rig, "0");
    for i in 1..20 {
        ask(&mut rig, &i.to_string());
    }
    let q = rig
        .s
        .suggest(NewSuggestion { tab_id: "implementa".into(), title: "20".into(), tldr: "".into(), prompt: "p".into() })
        .unwrap();
    assert_eq!(q.evicted.len(), 1);
    assert_eq!(q.evicted[0].id, first);
    assert_eq!(q.pending.len(), 20);
    assert_eq!(rig.s.settled_as(&first), Some(SuggestionEnd::Dismissed));
}

#[test]
fn suggestion_of_follows_the_card() {
    let mut rig = fixture();
    let id = ask(&mut rig, "Converter SQL");
    let n = rig.s.notices()[0].id;
    assert_eq!(rig.s.suggestion_of(n).unwrap().id, id);
    assert_eq!(rig.s.notices()[0].notice.detail.as_deref(), Some("Converter SQL"));
    rig.s.dismiss_suggestion("implementa", &id);
    assert!(rig.s.suggestion_of(n).is_none());
    assert_eq!(rig.s.notices()[0].notice.detail.as_deref(), Some("Converter SQL"));
    rig.turn("implementa");
    assert!(rig.s.suggestion_of(rig.s.notices()[0].id).is_none());
}

#[test]
fn run_here_needs_a_live_process() {
    let mut rig = fixture();
    let id = ask(&mut rig, "x");
    rig.tm("implementa").exited = true;
    assert!(!rig.s.run_suggestion_here("implementa", &id, None));
    assert_eq!(rig.s.banner(), Some("implementa não está rodando — retome a conversa antes"));
    rig.tm("implementa").exited = false;
    assert!(rig.s.run_suggestion_here("implementa", &id, None));
    assert_eq!(rig.s.settled_as(&id), Some(SuggestionEnd::Started));
    assert_eq!(rig.terms.log("implementa").submitted, ["faz x"]);
}

#[test]
fn start_here_opens_a_sibling_session() {
    let mut rig = fixture();
    let id = ask(&mut rig, "Converter SQL");
    let tab = rig.s.start_suggestion_here("implementa", &id, None, None).unwrap();
    assert_eq!(rig.t(&tab).custom_label.as_deref(), Some("Converter SQL"));
    assert!(rig.terms.log(&tab).commands[0].ends_with("'faz Converter SQL'"));
    assert_eq!(rig.s.focused_pane_id(), Some(tab.as_str()));
    assert_eq!(rig.s.settled_as(&id), Some(SuggestionEnd::Started));
}

#[tokio::test]
async fn the_mcp_sink_reaches_the_store_through_the_channel() {
    let mut rig = fixture();
    let sink = rig.s.sink();
    let call = tokio::spawn(async move {
        sink.suggest(NewSuggestion { tab_id: "implementa".into(), title: "t".into(), tldr: "".into(), prompt: "p".into() })
            .await
    });
    while !call.is_finished() {
        rig.s.pump();
        tokio::task::yield_now().await;
    }
    assert!(call.await.unwrap().is_some());
    assert_eq!(rig.t("implementa").hooks.suggestions.len(), 1);
}
