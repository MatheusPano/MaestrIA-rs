use mx_core::layout::{Axis, PaneNode};
use mx_core::persist::PaneOpen;
use serde_json::json;

use super::Rig;
use crate::How;

fn rig(mode: PaneOpen, ids: &[&str]) -> Rig {
    let mut rig = Rig::new();
    for id in ids {
        rig.panel(id);
    }
    rig.s.set_pane_open(mode);
    rig
}

fn leaf(id: &str) -> PaneNode {
    PaneNode::leaf(id)
}

fn pair(axis: Axis, a: &str, b: &str) -> PaneNode {
    PaneNode::split(axis, vec![leaf(a), leaf(b)], vec![0.5, 0.5])
}

fn open(rig: &mut Rig, id: &str) {
    rig.s.select_with(id, How::FromSetting);
}

#[test]
fn replace_swaps_the_focused_pane_like_before() {
    let mut rig = rig(PaneOpen::Replace, &["a", "b"]);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&leaf("b")));
}

#[test]
fn beside_splits_the_focused_pane_to_the_right() {
    let mut rig = rig(PaneOpen::Beside, &["a", "b"]);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "a", "b")));
    assert_eq!(rig.s.focused_pane_id(), Some("b"));
}

#[test]
fn below_splits_the_focused_pane_downwards() {
    let mut rig = rig(PaneOpen::Below, &["a", "b"]);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Column, "a", "b")));
}

#[test]
fn auto_without_a_known_size_goes_to_the_right() {
    let mut rig = rig(PaneOpen::Auto, &["a", "b"]);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "a", "b")));
}

#[test]
fn auto_in_a_wide_area_goes_to_the_right() {
    let mut rig = rig(PaneOpen::Auto, &["a", "b"]);
    rig.s.set_pane_area(1600., 900.);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "a", "b")));
}

#[test]
fn auto_in_a_tall_area_goes_below() {
    let mut rig = rig(PaneOpen::Auto, &["a", "b", "c"]);
    rig.s.set_pane_area(1600., 900.);
    rig.s.set_panes(Some(pair(Axis::Row, "a", "b")), Some("b"));
    open(&mut rig, "c");
    let right = pair(Axis::Column, "b", "c");
    assert_eq!(rig.s.panes(), Some(&PaneNode::split(Axis::Row, vec![leaf("a"), right], vec![0.5, 0.5])));
}

#[test]
fn mouse_on_the_left_half_of_the_focused_pane_opens_on_the_left() {
    let mut rig = rig(PaneOpen::Mouse, &["a", "b"]);
    rig.s.set_pane_area(1600., 900.);
    rig.s.set_pane_cursor(Some((300., 450.)));
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "b", "a")));
    assert_eq!(rig.s.focused_pane_id(), Some("b"));
}

#[test]
fn mouse_on_the_right_half_opens_on_the_right() {
    let mut rig = rig(PaneOpen::Mouse, &["a", "b"]);
    rig.s.set_pane_area(1600., 900.);
    rig.s.set_pane_cursor(Some((1300., 450.)));
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "a", "b")));
}

#[test]
fn mouse_on_the_top_half_of_a_tall_pane_opens_above() {
    let mut rig = rig(PaneOpen::Mouse, &["a", "b", "c"]);
    rig.s.set_pane_area(1600., 900.);
    rig.s.set_pane_cursor(Some((1200., 100.)));
    rig.s.set_panes(Some(pair(Axis::Row, "a", "b")), Some("b"));
    open(&mut rig, "c");
    let right = pair(Axis::Column, "c", "b");
    assert_eq!(rig.s.panes(), Some(&PaneNode::split(Axis::Row, vec![leaf("a"), right], vec![0.5, 0.5])));
}

#[test]
fn mouse_outside_the_split_pane_behaves_like_auto() {
    let mut rig = rig(PaneOpen::Mouse, &["a", "b", "c"]);
    rig.s.set_pane_area(1600., 900.);
    rig.s.set_pane_cursor(Some((100., 100.)));
    rig.s.set_panes(Some(pair(Axis::Row, "a", "b")), Some("b"));
    open(&mut rig, "c");
    let right = pair(Axis::Column, "b", "c");
    assert_eq!(rig.s.panes(), Some(&PaneNode::split(Axis::Row, vec![leaf("a"), right], vec![0.5, 0.5])));
}

#[test]
fn mouse_off_the_panes_behaves_like_auto() {
    let mut rig = rig(PaneOpen::Mouse, &["a", "b"]);
    rig.s.set_pane_area(1600., 900.);
    rig.s.set_pane_cursor(None);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "a", "b")));
}

#[test]
fn every_mode_only_focuses_a_tab_already_on_screen() {
    for mode in PaneOpen::ALL {
        let mut rig = rig(mode, &["a", "b"]);
        rig.s.set_pane_area(1600., 900.);
        rig.s.set_panes(Some(pair(Axis::Row, "a", "b")), Some("a"));
        open(&mut rig, "b");
        assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "a", "b")), "{mode:?}");
        assert_eq!(rig.s.focused_pane_id(), Some("b"), "{mode:?}");
    }
}

#[test]
fn every_mode_makes_the_only_pane_on_an_empty_screen() {
    for mode in PaneOpen::ALL {
        let mut rig = rig(mode, &["a"]);
        open(&mut rig, "a");
        assert_eq!(rig.s.panes(), Some(&leaf("a")), "{mode:?}");
        assert_eq!(rig.s.focused_pane_id(), Some("a"), "{mode:?}");
    }
}

#[test]
fn a_stale_focus_splits_the_first_pane() {
    let mut rig = rig(PaneOpen::Beside, &["a", "b", "c"]);
    rig.s.set_panes(Some(leaf("a")), Some("b"));
    open(&mut rig, "c");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "a", "c")));
}

#[test]
fn split_modes_may_split_a_pinned_pane() {
    let mut rig = rig(PaneOpen::Beside, &["a", "b"]);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    rig.tm("a").pinned = true;
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "a", "b")));
    assert!(rig.t("a").pinned);
}

#[test]
fn beside_falls_back_to_replace_when_the_halves_would_be_too_narrow() {
    let mut rig = rig(PaneOpen::Beside, &["a", "b"]);
    rig.s.set_pane_area(400., 900.);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&leaf("b")));
}

#[test]
fn below_falls_back_to_replace_when_the_halves_would_be_too_short() {
    let mut rig = rig(PaneOpen::Below, &["a", "b"]);
    rig.s.set_pane_area(1600., 250.);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    open(&mut rig, "b");
    assert_eq!(rig.s.panes(), Some(&leaf("b")));
}

#[test]
fn auto_falls_back_to_replace_when_nothing_fits() {
    let mut rig = rig(PaneOpen::Auto, &["a", "b", "c"]);
    rig.s.set_pane_area(600., 250.);
    rig.s.set_panes(Some(pair(Axis::Row, "a", "b")), Some("a"));
    open(&mut rig, "c");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "c", "b")));
}

#[test]
fn a_new_shell_follows_the_setting() {
    let mut rig = rig(PaneOpen::Below, &["a"]);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    let folder = rig.s1();
    let id = rig.s.open_shell(&folder, None, None, None, None);
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Column, "a", &id)));
    assert_eq!(rig.s.focused_pane_id(), Some(id.as_str()));
}

#[test]
fn cycling_sessions_still_replaces() {
    let mut rig = rig(PaneOpen::Beside, &["a", "b"]);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    rig.s.cycle(1);
    assert_eq!(rig.s.panes(), Some(&leaf("b")));
}

#[test]
fn replace_keeps_the_pinned_pane_and_swaps_the_loose_one() {
    let mut rig = rig(PaneOpen::Replace, &["fixo", "solto", "tres"]);
    rig.s.set_panes(Some(pair(Axis::Row, "fixo", "solto")), Some("fixo"));
    rig.tm("fixo").pinned = true;
    open(&mut rig, "tres");
    assert_eq!(rig.s.order(), ["fixo", "tres"]);
}

#[test]
fn replace_with_every_pane_pinned_still_adds_one_to_the_right_without_a_size() {
    let mut rig = rig(PaneOpen::Replace, &["fixo", "novo"]);
    rig.s.set_panes(Some(leaf("fixo")), Some("fixo"));
    rig.tm("fixo").pinned = true;
    open(&mut rig, "novo");
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Row, "fixo", "novo")));
}

#[test]
fn every_pane_pinned_splits_the_largest_by_its_shape() {
    let mut rig = rig(PaneOpen::Replace, &["a", "b", "novo"]);
    rig.s.set_pane_area(1000., 1200.);
    let tree = PaneNode::split(Axis::Row, vec![leaf("a"), leaf("b")], vec![0.3, 0.7]);
    rig.s.set_panes(Some(tree), Some("a"));
    rig.tm("a").pinned = true;
    rig.tm("b").pinned = true;
    open(&mut rig, "novo");
    let right = pair(Axis::Column, "b", "novo");
    assert_eq!(rig.s.panes(), Some(&PaneNode::split(Axis::Row, vec![leaf("a"), right], vec![0.3, 0.7])));
}

#[test]
fn replace_refills_the_hole_a_loose_pane_left() {
    let mut rig = rig(PaneOpen::Replace, &["fixo", "solto", "novo"]);
    let tree = PaneNode::split(Axis::Row, vec![leaf("fixo"), leaf("solto")], vec![0.3, 0.7]);
    rig.s.set_panes(Some(tree), Some("solto"));
    rig.tm("fixo").pinned = true;
    rig.s.dismiss("solto");
    open(&mut rig, "novo");
    assert_eq!(rig.s.panes(), Some(&PaneNode::split(Axis::Row, vec![leaf("fixo"), leaf("novo")], vec![0.3, 0.7])));
}

#[test]
fn a_plugin_view_beside_follows_the_pane_shape() {
    let mut rig = rig(PaneOpen::Replace, &["a", "b"]);
    rig.s.set_pane_area(800., 1200.);
    rig.s.set_panes(Some(leaf("a")), Some("a"));
    rig.s.place_beside("b", Some("a"));
    assert_eq!(rig.s.panes(), Some(&pair(Axis::Column, "a", "b")));
}

#[test]
fn the_setting_round_trips_and_replace_is_never_written() {
    let mut rig = rig(PaneOpen::Auto, &[]);
    assert_eq!(rig.written()["paneOpen"], json!("auto"));
    rig.s.set_pane_open(PaneOpen::Replace);
    assert!(rig.written().get("paneOpen").is_none());
    rig.s.set_pane_open(PaneOpen::Below);
    let saved = rig.written();
    let mut back = Rig::new();
    let state = back.s.opts.state_dir.clone();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("config.json"), saved.to_string()).unwrap();
    back.s.load();
    assert_eq!(back.s.prefs().pane_open, PaneOpen::Below);
}

#[test]
fn the_pane_area_is_remembered() {
    let mut rig = Rig::new();
    assert_eq!(rig.s.pane_area(), None);
    rig.s.set_pane_area(1200., 700.);
    assert_eq!(rig.s.pane_area(), Some((1200., 700.)));
}

#[test]
fn focus_following_the_mouse_is_a_saved_preference() {
    let mut rig = Rig::new();
    assert!(!rig.s.prefs().focus_follows_mouse);
    rig.s.set_focus_follows_mouse(true);
    assert_eq!(rig.written()["focusFollowsMouse"], json!(true));
    rig.s.set_focus_follows_mouse(false);
    assert!(rig.written().get("focusFollowsMouse").is_none());
}
