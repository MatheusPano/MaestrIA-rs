use mx_core::layout::{Axis, DropSide, PaneNode};
use mx_core::model::Doc;

use super::Rig;
use crate::DocPlace;

fn rig(ids: &[&str]) -> Rig {
    let mut rig = Rig::new();
    for id in ids {
        rig.panel(id);
    }
    rig
}

fn row(a: &str, b: &str) -> PaneNode {
    PaneNode::split(Axis::Row, vec![PaneNode::leaf(a), PaneNode::leaf(b)], vec![0.5, 0.5])
}

fn pin(rig: &mut Rig, id: &str) {
    rig.tm(id).pinned = true;
}

#[test]
fn n1_select_skips_the_pinned_focused_pane() {
    let mut rig = rig(&["fixo", "solto", "tres"]);
    rig.s.set_panes(Some(row("fixo", "solto")), Some("fixo"));
    pin(&mut rig, "fixo");
    rig.s.select("tres");
    assert_eq!(rig.s.order(), ["fixo", "tres"]);
    assert_eq!(rig.s.focused_pane_id(), Some("tres"));
    assert!(rig.t("fixo").pinned);
}

#[test]
fn n2_select_replaces_the_focused_loose_pane() {
    let mut rig = rig(&["fixo", "solto", "tres"]);
    rig.s.set_panes(Some(row("fixo", "solto")), Some("solto"));
    pin(&mut rig, "fixo");
    rig.s.select("tres");
    assert_eq!(rig.s.order(), ["fixo", "tres"]);
}

#[test]
fn n3_with_only_pinned_panes_select_adds_one() {
    let mut rig = rig(&["fixo", "novo"]);
    rig.s.look("fixo");
    pin(&mut rig, "fixo");
    rig.s.select("novo");
    assert_eq!(rig.s.order(), ["fixo", "novo"]);
    assert_eq!(rig.s.focused_pane_id(), Some("novo"));
}

#[test]
fn n4_dismissing_a_pinned_pane_unpins_it() {
    let mut rig = rig(&["fixo", "solto"]);
    rig.s.set_panes(Some(row("fixo", "solto")), Some("fixo"));
    pin(&mut rig, "fixo");
    rig.s.dismiss("fixo");
    assert!(!rig.t("fixo").pinned);
    assert!(!rig.s.is_pinned("fixo"));
}

#[test]
fn n5_dropping_onto_a_pinned_pane_takes_its_pin() {
    let mut rig = rig(&["fixo", "solto", "tres"]);
    rig.s.set_panes(Some(row("fixo", "solto")), Some("fixo"));
    pin(&mut rig, "fixo");
    rig.s.drop_tab("tres", "fixo", DropSide::Center);
    assert_eq!(rig.s.order(), ["tres", "solto"]);
    assert!(rig.t("tres").pinned);
    assert!(!rig.t("fixo").pinned);
}

#[test]
fn n6_the_hole_left_by_a_loose_pane_is_refilled_with_its_weights() {
    let mut rig = rig(&["solto", "a", "b", "outro"]);
    let col = PaneNode::split(Axis::Column, vec![PaneNode::leaf("a"), PaneNode::leaf("b")], vec![0.5, 0.5]);
    let tree = PaneNode::split(Axis::Row, vec![PaneNode::leaf("solto"), col], vec![0.4, 0.6]);
    rig.s.set_panes(Some(tree), Some("a"));
    pin(&mut rig, "a");
    pin(&mut rig, "b");
    rig.s.dismiss("solto");
    rig.s.select("outro");
    let root = rig.s.panes().unwrap().as_split().unwrap();
    assert_eq!(root.axis, Axis::Row);
    assert_eq!(root.weights, [0.4, 0.6]);
    assert_eq!(root.children[0], PaneNode::leaf("outro"));
    assert_eq!(root.children[1].order(), ["a", "b"]);
    assert!(rig.t("a").pinned && rig.t("b").pinned);
}

#[test]
fn n7_closing_a_loose_pane_leaves_a_hole_too() {
    let mut rig = rig(&["solto", "fixo", "outro"]);
    rig.s.set_panes(Some(row("solto", "fixo")), Some("solto"));
    pin(&mut rig, "fixo");
    rig.s.close_tab("solto");
    rig.s.select("outro");
    assert_eq!(rig.s.order(), ["outro", "fixo"]);
}

#[test]
fn n8_a_changed_shape_forgets_the_hole() {
    let mut rig = rig(&["solto", "a", "b", "outro"]);
    rig.s.set_panes(Some(row("solto", "a")), Some("a"));
    pin(&mut rig, "a");
    rig.s.dismiss("solto");
    rig.s.drop_tab("b", "a", DropSide::Bottom);
    rig.s.toggle_pin("b");
    rig.s.focus_pane("a");
    rig.s.select("outro");
    let root = rig.s.panes().unwrap().as_split().unwrap();
    assert_eq!(root.axis, Axis::Row);
    assert_eq!(root.children[0].order(), ["a", "b"]);
    assert_eq!(root.children[1], PaneNode::leaf("outro"));
}

#[test]
fn n9_every_pane_pinned_opens_at_the_edge() {
    let mut rig = rig(&["a", "b", "novo"]);
    let col = PaneNode::split(Axis::Column, vec![PaneNode::leaf("a"), PaneNode::leaf("b")], vec![0.5, 0.5]);
    rig.s.set_panes(Some(col), Some("a"));
    pin(&mut rig, "a");
    pin(&mut rig, "b");
    rig.s.select("novo");
    let root = rig.s.panes().unwrap().as_split().unwrap();
    assert_eq!(root.axis, Axis::Row);
    assert_eq!(root.children[0].as_split().unwrap().axis, Axis::Column);
    assert_eq!(root.children[0].order(), ["a", "b"]);
    assert_eq!(root.children[1], PaneNode::leaf("novo"));
}

#[test]
fn n10_a_doc_opens_beside_the_first_loose_pane() {
    let mut rig = rig(&["solto", "fixo"]);
    rig.s.set_panes(Some(row("solto", "fixo")), Some("fixo"));
    pin(&mut rig, "fixo");
    let now = rig.now();
    let doc = rig.s.show_doc(Doc::file("/repo/README.md", None, now), DocPlace::from_tab("fixo"));
    assert_eq!(rig.s.order(), ["solto".to_owned(), doc, "fixo".to_owned()]);
    let root = rig.s.panes().unwrap().as_split().unwrap();
    assert_eq!(*root.weights.last().unwrap(), 0.5);
}

#[test]
fn n11_pinned_is_saved_in_the_layout_but_not_the_recipe() {
    let mut rig = rig(&["fixo"]);
    rig.s.look("fixo");
    rig.s.toggle_pin("fixo");
    assert_eq!(rig.t("fixo").to_json()["pinned"], true);
    assert!(rig.t("fixo").recipe().get("pinned").is_none());
}
