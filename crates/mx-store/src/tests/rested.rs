use chrono::TimeDelta;
use mx_core::model::{ClaudeStatus, FeatureOrHotfixKind, FollowUp, FollowUpKind, TabKind};
use serde_json::json;

use super::Rig;

const T1: &str = "implementa";

fn fixture() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel(T1);
    rig
}

#[test]
fn e2_a_turn_off_screen_rests_unseen() {
    let mut rig = fixture();
    rig.turn(T1);
    let t = rig.t(T1);
    assert!(t.rested_at.is_some() && t.rested() && t.unseen);
    assert_eq!(t.rested_ago(&rig.now()).as_deref(), Some("agora"));
}

#[test]
fn e3_watched_rest_is_seen() {
    let mut rig = fixture();
    rig.s.look(T1);
    rig.turn(T1);
    assert!(rig.t(T1).rested() && !rig.t(T1).unseen);
}

#[test]
fn e4_an_inactive_window_does_not_watch() {
    let mut rig = fixture();
    rig.s.look(T1);
    rig.s.set_window_active(false);
    rig.turn(T1);
    assert!(rig.t(T1).unseen);
}

#[test]
fn e5_a_new_prompt_leaves_rest() {
    let mut rig = fixture();
    rig.turn(T1);
    rig.hook(T1, "UserPromptSubmit", json!({}));
    let t = rig.t(T1);
    assert!(t.rested_at.is_none() && !t.rested() && !t.unseen);
}

#[test]
fn e6_a_second_stop_keeps_the_timestamp() {
    let mut rig = fixture();
    rig.turn(T1);
    let at = rig.t(T1).rested_at;
    rig.clock.advance(TimeDelta::seconds(30));
    rig.hook(T1, "Stop", json!({}));
    assert_eq!(rig.t(T1).rested_at, at);
}

#[test]
fn e7_see_focused_clears_unseen_only_when_watching() {
    let mut rig = fixture();
    rig.turn(T1);
    assert!(!rig.s.see_focused());
    assert!(rig.t(T1).unseen);
    rig.s.look(T1);
    rig.s.set_window_active(false);
    assert!(!rig.s.see_focused());
    rig.s.set_window_active(true);
    assert!(rig.s.see_focused());
    assert!(!rig.t(T1).unseen);
    assert!(!rig.s.see_focused());
}

#[test]
fn e8_returning_with_a_toast_shows_no_banner() {
    let mut rig = fixture();
    rig.s.set_window_active(false);
    rig.turn(T1);
    rig.s.set_window_active(true);
    assert_eq!(rig.s.toasts().len(), 1);
    assert!(rig.s.banner().is_none());
    assert!(rig.t(T1).unseen);
}

#[test]
fn e9_returning_without_toasts_tells_what_finished() {
    let mut rig = fixture();
    rig.s.set_window_active(false);
    rig.clock.advance(TimeDelta::seconds(1));
    rig.turn(T1);
    let id = rig.s.toasts()[0].id;
    rig.s.dismiss_toast(id);
    rig.s.set_window_active(true);
    assert_eq!(rig.s.banner(), Some("implementa terminou enquanto você estava fora"));
    assert!(rig.t(T1).unseen);
}

#[test]
fn e10_rest_before_leaving_is_not_news() {
    let mut rig = fixture();
    rig.turn(T1);
    rig.s.clear_banner();
    rig.s.set_window_active(false);
    rig.s.set_window_active(true);
    assert!(rig.t(T1).unseen);
    assert!(rig.s.banner().is_none());
}

#[test]
fn several_finished_while_away_are_counted() {
    let mut rig = fixture();
    rig.panel("outro");
    rig.s.set_do_not_disturb(true);
    rig.s.set_window_active(false);
    rig.clock.advance(TimeDelta::seconds(1));
    rig.turn(T1);
    rig.turn("outro");
    rig.s.set_do_not_disturb(false);
    rig.s.set_window_active(true);
    assert_eq!(rig.s.banner(), Some("2 painéis terminaram enquanto você estava fora"));
}

#[test]
fn d1_d2_d3_d4_done_mark() {
    let mut rig = fixture();
    assert!(rig.t(T1).to_json().get("done").is_none());
    let step = |t: &str| FollowUp { kind: FollowUpKind::KeepGoing, text: t.into(), target_tab_id: None };
    rig.s.queue(T1, vec![step("um"), step("dois")], false);
    assert_eq!(rig.s.set_done(T1, true), 2);
    assert!(rig.t(T1).done && rig.t(T1).follow_ups.is_empty());
    assert_eq!(rig.s.tabs().len(), 1);
    assert_eq!(rig.t(T1).to_json()["done"], true);
    assert_eq!(rig.s.set_done(T1, true), 0);
    assert!(rig.t(T1).done);
}

#[test]
fn d5_done_panels_stop_needing_a_human() {
    let mut rig = fixture();
    let f = rig.s.add_feature("/repo", "x", "", FeatureOrHotfixKind::Feature);
    rig.s.assign(T1, Some(&f));
    rig.tm(T1).hooks.status = ClaudeStatus::WaitingPermission;
    assert_eq!((rig.s.needing_human("/repo"), rig.s.needing_human_in(&f)), (1, 1));
    rig.s.set_done(T1, true);
    assert_eq!((rig.s.needing_human("/repo"), rig.s.needing_human_in(&f)), (0, 0));
}

#[test]
fn d6_d7_a_prompt_clears_done_and_a_turn_never_sets_it() {
    let mut rig = fixture();
    rig.s.set_done(T1, true);
    rig.hook(T1, "UserPromptSubmit", json!({}));
    assert!(!rig.t(T1).done);
    rig.hook(T1, "Stop", json!({}));
    assert_eq!(rig.t(T1).status(), ClaudeStatus::Idle);
    assert!(!rig.t(T1).done);
}

#[test]
fn d8_close_settled_sweeps_only_done_panels() {
    let mut rig = fixture();
    rig.panel("pending");
    rig.s.set_done(T1, true);
    assert_eq!(rig.s.close_settled("/repo"), 1);
    let left: Vec<&str> = rig.s.tabs_of("/repo").iter().map(|t| t.id.as_str()).collect();
    assert_eq!(left, ["pending"]);
}

#[test]
fn d9_undone_needs_a_human_again() {
    let mut rig = fixture();
    rig.tm(T1).hooks.status = ClaudeStatus::WaitingPermission;
    rig.s.set_done(T1, true);
    assert_eq!(rig.s.set_done(T1, false), 0);
    assert!(!rig.t(T1).done);
    assert_eq!(rig.s.needing_human("/repo"), 1);
}

#[test]
fn loose_tray_sweep_keeps_running_and_exited_panels() {
    let mut rig = Rig::new();
    let home = rig.s.loose().root.clone();
    rig.add_tab("a", &home, TabKind::Shell, &home, Some("ainda roda"));
    rig.add_tab("b", &home, TabKind::Shell, &home, Some("acabou"));
    rig.add_tab("c", &home, TabKind::Claude, &home, Some("saiu na largada"));
    rig.tm("c").exited = true;
    rig.s.set_done("b", true);
    assert_eq!(rig.s.settled_in(&home), 1);
    assert_eq!(rig.s.close_settled(&home), 1);
    let titles: Vec<String> = rig.s.tabs_of(&home).iter().map(|t| rig.s.title(t)).collect();
    assert_eq!(titles, ["ainda roda", "saiu na largada"]);
    assert_eq!(rig.s.close_settled(&home), 0);
}
