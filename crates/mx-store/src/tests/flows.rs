use chrono::TimeDelta;
use mx_core::model::{ClaudeStatus, FlowHold, FollowUp, FollowUpKind};
use serde_json::json;

use super::Rig;
use crate::handoff_text;

fn fixture() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("implementa");
    rig
}

const T1: &str = "implementa";

fn step(text: &str) -> FollowUp {
    FollowUp { kind: FollowUpKind::KeepGoing, text: text.into(), target_tab_id: None }
}

fn steps(texts: &[&str]) -> Vec<FollowUp> {
    texts.iter().map(|t| step(t)).collect()
}

fn quiet(rig: &mut Rig) {
    rig.silent(T1, TimeDelta::seconds(5));
}

fn left(rig: &Rig) -> usize {
    rig.t(T1).follow_ups.len()
}

#[test]
fn q1_each_rest_fires_one_step() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["um", "dois"]), false);
    rig.turn(T1);
    quiet(&mut rig);
    rig.s.pump_flows();
    assert_eq!(rig.t(T1).follow_ups[0].text, "dois");
    assert_eq!(rig.terms.log(T1).submitted, ["um"]);
    rig.turn(T1);
    quiet(&mut rig);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 0);
}

#[test]
fn q2_a_second_stop_does_not_fire_twice() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["um", "dois"]), false);
    rig.hook(T1, "UserPromptSubmit", json!({}));
    rig.hook(T1, "Stop", json!({}));
    rig.hook(T1, "Stop", json!({}));
    quiet(&mut rig);
    rig.s.pump_flows();
    rig.s.pump_flows();
    assert_eq!(left(&rig), 1);
}

#[test]
fn q3_forks_hold_the_flow_until_they_report() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["revisa"]), false);
    rig.hook(T1, "UserPromptSubmit", json!({}));
    rig.hook(T1, "PreToolUse", json!({"tool_name": "Agent"}));
    rig.hook(T1, "PreToolUse", json!({"tool_name": "Agent"}));
    rig.hook(T1, "Stop", json!({}));
    quiet(&mut rig);
    assert_eq!(rig.t(T1).status(), ClaudeStatus::Idle);
    assert_eq!(rig.s.hold_for(T1), FlowHold::Forks);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 1);
    rig.hook(T1, "PostToolUse", json!({"agent_id": "a1"}));
    rig.hook(T1, "SubagentStop", json!({"agent_id": "a1"}));
    quiet(&mut rig);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 1);
    rig.hook(T1, "SubagentStop", json!({"agent_id": "a2"}));
    quiet(&mut rig);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 0);
}

#[test]
fn q4_idle_prompt_rest_also_counts() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["revisa"]), false);
    rig.hook(T1, "UserPromptSubmit", json!({}));
    rig.hook(T1, "PreToolUse", json!({"tool_name": "Agent"}));
    rig.hook(T1, "Stop", json!({}));
    rig.hook(T1, "Notification", json!({"notification_type": "idle_prompt"}));
    assert!(rig.t(T1).status().at_rest());
    rig.hook(T1, "SubagentStop", json!({"agent_id": "a1"}));
    quiet(&mut rig);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 0);
}

#[test]
fn q5_patience_runs_out_on_silent_forks() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["segue"]), false);
    rig.hook(T1, "UserPromptSubmit", json!({}));
    rig.hook(T1, "PreToolUse", json!({"tool_name": "Task"}));
    rig.hook(T1, "Stop", json!({}));
    rig.silent(T1, TimeDelta::minutes(3));
    assert_eq!(rig.s.hold_for(T1), FlowHold::Go);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 0);
}

#[test]
fn q6_a_fresh_rest_waits_for_quiet() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["um"]), false);
    rig.turn(T1);
    assert_eq!(rig.s.hold_for(T1), FlowHold::Quiet);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 1);
    quiet(&mut rig);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 0);
}

#[test]
fn q7_handoff_submits_to_the_target_and_keeps_going() {
    let mut rig = fixture();
    rig.panel("t2");
    let hand = FollowUp { kind: FollowUpKind::Handoff, text: "assume".into(), target_tab_id: Some("t2".into()) };
    rig.s.queue(T1, vec![hand, step("depois")], false);
    rig.turn(T1);
    quiet(&mut rig);
    rig.s.pump_flows();
    let kinds: Vec<FollowUpKind> = rig.t(T1).follow_ups.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [FollowUpKind::KeepGoing]);
    assert!(rig.t(T1).armed);
    assert!(rig.terms.log("t2").submitted[0].contains("assume"));
    quiet(&mut rig);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 0);
}

#[test]
fn q8_command_step_opens_a_child_shell_right_after_the_parent() {
    let mut rig = fixture();
    rig.panel("t9");
    let cmd = FollowUp { kind: FollowUpKind::Command, text: "flutter analyze && flutter test".into(), target_tab_id: None };
    rig.s.queue(T1, vec![cmd], false);
    rig.turn(T1);
    quiet(&mut rig);
    rig.s.pump_flows();
    let ids = rig.ids();
    let child = ids[1].clone();
    assert_eq!(rig.t(&child).born_of.as_deref(), Some(T1));
    assert!(rig.s.descends_from(&child, T1));
    assert_eq!(ids.last().unwrap(), "t9");
    assert_eq!(rig.s.title(rig.t(&child)), "flutter analyze && flutter…");
    assert_eq!(rig.terms.log(&child).commands, ["flutter analyze && flutter test"]);
}

#[test]
fn q9_no_queue_nothing_fires() {
    let mut rig = fixture();
    rig.turn(T1);
    quiet(&mut rig);
    rig.s.pump_flows();
    assert!(!rig.t(T1).armed && left(&rig) == 0);
}

#[test]
fn q10_queue_replaces_the_steps() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["velho"]), false);
    rig.s.queue(T1, steps(&["novo", "outro"]), false);
    let texts: Vec<&str> = rig.t(T1).follow_ups.iter().map(|f| f.text.as_str()).collect();
    assert_eq!(texts, ["novo", "outro"]);
}

#[test]
fn q11_queueing_while_resting_waits_unless_now() {
    let mut rig = fixture();
    rig.turn(T1);
    quiet(&mut rig);
    rig.s.queue(T1, steps(&["vai"]), false);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 1);
    rig.s.queue(T1, steps(&["vai"]), true);
    rig.s.pump_flows();
    assert_eq!(left(&rig), 0);
}

#[test]
fn q12_closing_disarms() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["um"]), false);
    rig.turn(T1);
    assert!(rig.t(T1).armed);
    let gone = rig.s.close_tab(T1).unwrap();
    rig.s.pump_flows();
    assert!(!gone.armed);
    assert_eq!(gone.follow_ups.len(), 1);
}

#[test]
fn q13_done_drops_the_queue() {
    let mut rig = fixture();
    rig.s.queue(T1, steps(&["um", "dois"]), false);
    rig.turn(T1);
    assert_eq!(rig.s.set_done(T1, true), 2);
    quiet(&mut rig);
    rig.s.pump_flows();
    assert!(left(&rig) == 0 && !rig.t(T1).armed);
}

#[test]
fn q14_q15_handoff_text() {
    let said = "Troquei o endpoint pra /v2/permissions.\nFalta o refresh token.";
    let t = handoff_text("implementa", Some(said), "Pega daqui e termina o refresh.");
    assert!(t.contains("\"implementa\"") && t.contains("Falta o refresh token.") && t.contains("Pega daqui"));
    assert!(handoff_text("implementa", None, "assume").contains("assume"));
    let t = handoff_text("implementa", Some("pronto"), "");
    assert!(t.contains("pronto") && t.trim().ends_with("\"\"\""));
    let long = "x".repeat(5000);
    let t = handoff_text("a", Some(&long), "");
    assert!(t.contains(&format!("…{}", "x".repeat(4000))) && !t.contains(&"x".repeat(4001)));
}

#[test]
fn new_session_step_opens_a_claude_child() {
    let mut rig = fixture();
    let next = FollowUp { kind: FollowUpKind::NewSession, text: "revisa".into(), target_tab_id: None };
    rig.s.queue(T1, vec![next], false);
    rig.turn(T1);
    quiet(&mut rig);
    rig.s.pump_flows();
    let child = rig.ids()[1].clone();
    assert_eq!(rig.s.title(rig.t(&child)), "implementa ▸ depois");
    assert!(rig.terms.log(&child).commands[0].ends_with("'revisa'"));
    assert_eq!(rig.s.banner(), Some("implementa terminou — abri implementa ▸ depois em seguida"));
}

#[test]
fn start_flow_announces_the_steps() {
    let mut rig = fixture();
    let f = rig.s.find_folder("/repo").unwrap().clone();
    let id = rig.s.start_flow(&f, "faz", steps(&["a", "b"]), None, Some("job"), None);
    assert_eq!(rig.t(&id).follow_ups.len(), 2);
    assert_eq!(rig.s.banner(), Some("job: fluxo de 2 passos armado"));
}
