use chrono::TimeDelta;
use mx_core::model::{ClaudeStatus, FollowUp, FollowUpKind};

use super::Rig;

fn fixture() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.add_tab("t1", "/repo", mx_core::model::TabKind::Claude, "/repo", Some("implementa"));
    rig.tm("t1").session_id = Some("sess-1".into());
    rig
}

fn step(text: &str) -> FollowUp {
    FollowUp { kind: FollowUpKind::KeepGoing, text: text.into(), target_tab_id: None }
}

fn rested(rig: &mut Rig, ago: TimeDelta) {
    rig.turn("t1");
    let now = rig.now();
    rig.tm("t1").rested_at = Some(now - ago);
}

#[test]
fn h1_hibernate_parks_a_resumable_session() {
    let mut rig = fixture();
    assert!(rig.t("t1").can_hibernate());
    rig.s.hibernate("t1");
    let t = rig.t("t1");
    assert!(t.hibernated && t.exited && t.resumable());
    assert_eq!(t.resume_id(), Some("sess-1"));
    assert!(rig.s.subtitle(t).contains("Hibernada"));
    assert_eq!(t.status(), ClaudeStatus::Ended);
    assert!(!t.can_hibernate());
    assert_eq!(rig.s.tabs().len(), 1);
    assert_eq!(rig.terms.log("t1").remarks, [crate::HIBERNATED]);
}

#[test]
fn h2_without_session_nothing_happens() {
    let mut rig = fixture();
    rig.tm("t1").session_id = None;
    assert!(!rig.t("t1").can_hibernate());
    rig.s.hibernate("t1");
    assert!(!rig.t("t1").hibernated && !rig.t("t1").exited);
}

#[test]
fn h3_shells_never_hibernate() {
    let mut rig = fixture();
    rig.shell("s");
    assert!(!rig.t("s").can_hibernate());
}

#[test]
fn h4_hibernate_disarms_the_queue() {
    let mut rig = fixture();
    rig.tm("t1").armed = true;
    rig.s.hibernate("t1");
    assert!(!rig.t("t1").armed);
}

#[tokio::test]
async fn h5_select_wakes_the_session() {
    let mut rig = fixture();
    rig.s.hibernate("t1");
    rig.s.select("t1");
    rig.s.settle().await;
    let t = rig.t("t1");
    assert!(!t.hibernated);
    assert!(!rig.s.subtitle(t).contains("Hibernada"));
    assert_eq!(t.resume_id(), Some("sess-1"));
    assert!(rig.terms.log("t1").commands[0].contains("--resume 'sess-1'"));
}

#[test]
fn h6_waking_an_awake_tab_does_nothing() {
    let mut rig = fixture();
    let before = rig.t("t1").status();
    rig.s.wake("t1");
    assert_eq!(rig.t("t1").status(), before);
    assert!(!rig.t("t1").exited);
}

#[test]
fn h7_idle_off_screen_past_the_limit_hibernates() {
    let mut rig = fixture();
    rested(&mut rig, TimeDelta::minutes(31));
    let now = rig.now();
    rig.s.hibernate_idle(now);
    assert!(rig.t("t1").hibernated);
}

#[test]
fn h8_short_of_the_limit_stays_awake() {
    let mut rig = fixture();
    rested(&mut rig, TimeDelta::minutes(29));
    let now = rig.now();
    rig.s.hibernate_idle(now);
    assert!(!rig.t("t1").hibernated);
}

#[test]
fn h9_ready_without_rest_counts_from_start() {
    let mut rig = fixture();
    rig.tm("t1").hooks.status = ClaudeStatus::Ready;
    let at = rig.t("t1").started_at + TimeDelta::minutes(31);
    rig.s.hibernate_idle(at);
    assert!(rig.t("t1").hibernated);
}

#[test]
fn h10_on_screen_never_hibernates() {
    let mut rig = fixture();
    rested(&mut rig, TimeDelta::hours(2));
    rig.s.look("t1");
    let now = rig.now();
    rig.s.hibernate_idle(now);
    assert!(!rig.t("t1").hibernated);
}

#[test]
fn h11_waiting_permission_stays_awake() {
    let mut rig = fixture();
    rig.tm("t1").hooks.status = ClaudeStatus::WaitingPermission;
    let at = rig.now() + TimeDelta::hours(2);
    rig.s.hibernate_idle(at);
    assert!(!rig.t("t1").hibernated);
}

#[test]
fn h12_working_stays_awake() {
    let mut rig = fixture();
    let now = rig.now();
    rig.tm("t1").hooks.status = ClaudeStatus::Working;
    rig.tm("t1").hooks.last_event_at = Some(now - TimeDelta::hours(2));
    rig.s.hibernate_idle(now);
    assert!(!rig.t("t1").hibernated);
}

#[test]
fn h13_a_queue_keeps_it_awake() {
    let mut rig = fixture();
    rested(&mut rig, TimeDelta::hours(2));
    rig.tm("t1").follow_ups.push(step("um"));
    let now = rig.now();
    rig.s.hibernate_idle(now);
    assert!(!rig.t("t1").hibernated);
}

#[test]
fn h14_a_handoff_target_stays_awake() {
    let mut rig = fixture();
    rested(&mut rig, TimeDelta::hours(2));
    rig.panel("t2");
    rig.tm("t2").follow_ups.push(FollowUp {
        kind: FollowUpKind::Handoff,
        text: "assume".into(),
        target_tab_id: Some("t1".into()),
    });
    let now = rig.now();
    rig.s.hibernate_idle(now);
    assert!(!rig.t("t1").hibernated);
}

#[test]
fn h15_zero_minutes_turns_it_off() {
    let mut rig = fixture();
    rested(&mut rig, TimeDelta::days(1));
    rig.s.set_hibernate_minutes(0);
    assert!(!rig.s.auto_hibernate());
    let now = rig.now();
    rig.s.hibernate_idle(now);
    assert!(!rig.t("t1").hibernated);
}

#[test]
fn h16_a_shorter_limit_applies() {
    let mut rig = fixture();
    rig.s.set_hibernate_minutes(5);
    rested(&mut rig, TimeDelta::minutes(6));
    let now = rig.now();
    rig.s.hibernate_idle(now);
    assert!(rig.t("t1").hibernated);
}

#[test]
fn h17_hibernated_is_saved_in_the_layout_but_not_the_recipe() {
    let mut rig = fixture();
    rig.s.hibernate("t1");
    let j = rig.t("t1").to_json();
    assert_eq!((j["hibernated"].clone(), j["sessionId"].clone()), (true.into(), "sess-1".into()));
    assert!(rig.t("t1").recipe().get("hibernated").is_none());
}

#[test]
fn hibernate_with_a_live_process_kills_it() {
    let mut rig = fixture();
    rig.terms.give_pid("t1");
    rig.s.hibernate("t1");
    assert_eq!(rig.terms.log("t1").kills, 1);
}

#[tokio::test]
async fn a_tab_hibernated_again_before_its_kill_lands_is_not_relaunched() {
    let mut rig = fixture();
    rig.s.hibernate("t1");
    rig.tm("t1").exited = false;
    let launched = rig.terms.log("t1").commands.len();
    rig.s.wake("t1");
    rig.s.hibernate("t1");
    rig.s.settle().await;
    assert!(rig.t("t1").hibernated);
    assert_eq!(rig.terms.log("t1").commands.len(), launched);
}

#[tokio::test]
async fn a_woken_tab_whose_kill_lands_is_relaunched() {
    let mut rig = fixture();
    rig.s.hibernate("t1");
    rig.tm("t1").exited = false;
    let launched = rig.terms.log("t1").commands.len();
    rig.s.wake("t1");
    rig.s.settle().await;
    assert!(!rig.t("t1").hibernated);
    assert_eq!(rig.terms.log("t1").commands.len(), launched + 1);
}
