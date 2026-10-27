use chrono::TimeDelta;
use mx_core::model::NoticeKind;
use serde_json::json;

use super::Rig;

const A: &str = "implementa";
const B: &str = "revisa";

fn fixture() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel(A);
    rig.panel(B);
    rig.s.look(B);
    rig
}

fn kinds(rig: &Rig) -> Vec<NoticeKind> {
    rig.s.notices().iter().map(|n| n.notice.kind).collect()
}

#[test]
fn a_finished_turn_is_one_unread_notice() {
    let mut rig = fixture();
    rig.turn(A);
    assert_eq!(kinds(&rig), [NoticeKind::Finished]);
    assert_eq!(rig.s.unread_notices(), 1);
    rig.hook(A, "Stop", json!({}));
    assert_eq!(rig.s.notices().len(), 1);
}

#[test]
fn questions_and_permissions_stack_newest_first() {
    let mut rig = fixture();
    rig.hook(A, "PreToolUse", json!({"tool_name": "AskUserQuestion", "tool_input": {"questions": [{"question": "Receitas"}]}}));
    rig.s.look(A);
    rig.hook(B, "PreToolUse", json!({"tool_name": "Bash"}));
    rig.hook(B, "Notification", json!({"notification_type": "permission_prompt"}));
    assert_eq!(kinds(&rig), [NoticeKind::Permission, NoticeKind::Question]);
    rig.hook(B, "PostToolUse", json!({"tool_name": "Bash"}));
    assert!(rig.s.notices()[0].notice.read);
}

#[test]
fn a_watched_finish_is_read_and_not_toasted() {
    let mut rig = fixture();
    rig.turn(B);
    assert_eq!(rig.s.notices().len(), 1);
    assert_eq!(rig.s.unread_notices(), 0);
    assert!(rig.s.toasts().is_empty());
    rig.turn(A);
    assert_eq!(rig.s.toasts()[0].id, rig.s.notices()[0].id);
}

#[test]
fn opening_a_notice_focuses_its_panel() {
    let mut rig = fixture();
    rig.turn(A);
    rig.s.toggle_notices();
    let id = rig.s.notices()[0].id;
    rig.s.open_notice(id);
    assert_eq!(rig.s.focused_pane_id(), Some(A));
    assert_eq!(rig.s.unread_notices(), 0);
    assert!(!rig.s.bell().notices_open);
}

#[test]
fn prompting_again_retires_the_finished_notice() {
    let mut rig = fixture();
    rig.turn(A);
    rig.hook(A, "UserPromptSubmit", json!({}));
    assert_eq!(rig.s.unread_notices(), 0);
    assert_eq!(rig.s.notices().len(), 1);
    assert!(rig.s.toasts().is_empty());
}

#[test]
fn only_one_list_is_open_at_a_time() {
    let mut rig = fixture();
    rig.s.toggle_notices();
    rig.s.toggle_sessions();
    assert!(!rig.s.bell().notices_open && rig.s.bell().sessions_open);
    rig.s.toggle_notices();
    assert!(rig.s.bell().notices_open && !rig.s.bell().sessions_open);
}

#[test]
fn dismissing_a_toast_keeps_the_notice_unread() {
    let mut rig = fixture();
    rig.turn(A);
    let id = rig.s.toasts()[0].id;
    rig.s.dismiss_toast(id);
    assert!(rig.s.toasts().is_empty());
    assert_eq!(rig.s.unread_notices(), 1);
}

#[test]
fn do_not_disturb_silences_toasts_but_counts() {
    let mut rig = fixture();
    rig.turn(A);
    rig.s.set_do_not_disturb(true);
    assert!(rig.s.toasts().is_empty());
    rig.panel("c");
    rig.turn("c");
    assert!(rig.s.toasts().is_empty());
    assert_eq!(rig.s.unread_notices(), 2);
}

#[test]
fn opening_the_bell_clears_toasts() {
    let mut rig = fixture();
    rig.turn(A);
    rig.s.toggle_notices();
    assert!(rig.s.toasts().is_empty());
}

#[test]
fn a_held_toast_outlives_its_timer_until_released() {
    let mut rig = fixture();
    rig.turn(A);
    let id = rig.s.toasts()[0].id;
    rig.s.hold_toast(id, true);
    rig.advance(TimeDelta::seconds(16));
    assert_eq!(rig.s.toasts().len(), 1);
    rig.s.hold_toast(id, false);
    rig.advance(TimeDelta::seconds(8));
    assert!(rig.s.toasts().is_empty());
}

#[test]
fn toasts_only_tick_while_the_window_is_active() {
    let mut rig = fixture();
    rig.s.set_window_active(false);
    rig.turn(A);
    rig.advance(TimeDelta::seconds(30));
    assert_eq!(rig.s.toasts().len(), 1);
    rig.s.set_window_active(true);
    rig.advance(TimeDelta::seconds(8));
    assert!(rig.s.toasts().is_empty());
}

#[test]
fn at_most_three_toasts_and_fifty_notices() {
    let mut rig = fixture();
    for i in 0..60 {
        let id = format!("p{i}");
        rig.panel(&id);
        rig.turn(&id);
    }
    assert_eq!(rig.s.toasts().len(), 3);
    assert_eq!(rig.s.notices().len(), 50);
    assert_eq!(rig.s.notices()[0].notice.tab_id, "p59");
}

#[test]
fn tapping_a_toast_reads_and_focuses() {
    let mut rig = fixture();
    rig.turn(A);
    let id = rig.s.toasts()[0].id;
    rig.s.open_notice(id);
    assert_eq!(rig.s.focused_pane_id(), Some(A));
    assert!(rig.s.toasts().is_empty());
    assert_eq!(rig.s.unread_notices(), 0);
}

#[test]
fn waiting_sessions_raise_one_desktop_alert_and_a_badge() {
    let mut rig = fixture();
    rig.hook(A, "PreToolUse", json!({"tool_name": "Bash"}));
    rig.hook(A, "Notification", json!({"notification_type": "permission_prompt"}));
    rig.hook(A, "Notification", json!({"notification_type": "permission_prompt"}));
    let alerts = rig.notifier.alerts.lock().unwrap().clone();
    assert_eq!(alerts, [("MaestrIA".to_owned(), "implementa · quer aprovação".to_owned())]);
    assert_eq!(rig.notifier.badges.lock().unwrap().last().cloned(), Some(Some("1".into())));
    rig.hook(A, "PostToolUse", json!({"tool_name": "Bash"}));
    assert_eq!(rig.notifier.badges.lock().unwrap().last().cloned(), Some(None));
}

#[test]
fn hooks_emit_plugin_events_in_order() {
    let mut rig = fixture();
    rig.hook(A, "UserPromptSubmit", json!({}));
    assert_eq!(rig.bus.names(), ["hook", "session.status"]);
}
