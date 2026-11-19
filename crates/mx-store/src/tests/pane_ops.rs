use mx_core::layout::{Axis, DropSide, PaneNode};
use mx_core::persist::EqualizeShape;

use super::Rig;
use crate::PaneDrop;

fn rig(ids: &[&str]) -> Rig {
    let mut rig = Rig::new();
    for id in ids {
        rig.panel(id);
    }
    rig
}

fn row(ids: &[&str]) -> PaneNode {
    let n = ids.len() as f64;
    PaneNode::split(Axis::Row, ids.iter().map(|id| PaneNode::leaf(*id)).collect(), vec![1. / n; ids.len()])
}

fn weights(rig: &Rig) -> Vec<f64> {
    rig.s.panes().unwrap().as_split().unwrap().weights.clone()
}

#[test]
fn a_swap_drop_trades_places_and_keeps_both_panes() {
    let mut rig = rig(&["a", "b", "c"]);
    rig.s.set_panes(Some(row(&["a", "b", "c"])), Some("a"));
    rig.s.drop_tab("a", "c", PaneDrop::Swap);
    assert_eq!(rig.s.order(), ["c", "b", "a"]);
    assert_eq!(rig.s.focused_pane_id(), Some("a"));
    assert!(rig.s.save_pending());
}

#[test]
fn a_swap_keeps_each_pane_with_its_own_pin() {
    let mut rig = rig(&["fixo", "solto"]);
    rig.s.set_panes(Some(row(&["fixo", "solto"])), Some("solto"));
    rig.tm("fixo").pinned = true;
    rig.s.drop_tab("solto", "fixo", PaneDrop::Swap);
    assert_eq!(rig.s.order(), ["solto", "fixo"]);
    assert!(rig.t("fixo").pinned);
    assert!(!rig.t("solto").pinned);
}

#[test]
fn a_swap_from_outside_the_tree_replaces_like_before() {
    let mut rig = rig(&["fixo", "solto", "fora"]);
    rig.s.set_panes(Some(row(&["fixo", "solto"])), Some("solto"));
    rig.tm("fixo").pinned = true;
    rig.s.drop_tab("fora", "fixo", PaneDrop::Swap);
    assert_eq!(rig.s.order(), ["fora", "solto"]);
    assert!(rig.t("fora").pinned);
    assert!(!rig.t("fixo").pinned);
    assert!(rig.s.has_tab("fixo"));
}

#[test]
fn an_edge_drop_spans_the_whole_area() {
    let mut rig = rig(&["a", "b", "c"]);
    rig.s.set_panes(Some(row(&["a", "b"])), Some("a"));
    rig.s.drop_tab("c", "a", PaneDrop::Edge(DropSide::Bottom));
    let root = rig.s.panes().unwrap().as_split().unwrap();
    assert_eq!(root.axis, Axis::Column);
    assert_eq!(root.children[0].order(), ["a", "b"]);
    assert_eq!(root.children[1], PaneNode::leaf("c"));
    assert_eq!(rig.s.focused_pane_id(), Some("c"));
}

#[test]
fn moving_a_pane_to_the_edge_takes_it_out_of_its_old_place() {
    let mut rig = rig(&["a", "b", "c"]);
    rig.s.set_panes(Some(row(&["a", "b", "c"])), Some("b"));
    rig.s.drop_at_edge("b", DropSide::Left);
    assert_eq!(rig.s.order(), ["b", "a", "c"]);
    assert_eq!(rig.s.panes().unwrap().as_split().unwrap().axis, Axis::Row);
}

#[test]
fn the_only_pane_dropped_at_an_edge_stays_put() {
    let mut rig = rig(&["a"]);
    rig.s.look("a");
    rig.s.drop_at_edge("a", DropSide::Right);
    assert_eq!(rig.s.panes(), Some(&PaneNode::leaf("a")));
}

#[test]
fn equalize_panes_evens_out_the_whole_tree_and_saves() {
    let mut rig = rig(&["a", "b", "c"]);
    let col = PaneNode::split(Axis::Column, vec![PaneNode::leaf("b"), PaneNode::leaf("c")], vec![0.1, 0.9]);
    rig.s.set_panes(Some(PaneNode::split(Axis::Row, vec![PaneNode::leaf("a"), col], vec![0.2, 0.8])), Some("a"));
    rig.s.equalize_panes();
    assert_eq!(weights(&rig), [0.5, 0.5]);
    assert_eq!(rig.s.panes().unwrap().as_split().unwrap().children[1].as_split().unwrap().weights, [0.5, 0.5]);
    assert!(rig.s.save_pending());
}

fn columns_grid() -> PaneNode {
    let col = |a: &str, b: &str| PaneNode::split(Axis::Column, vec![PaneNode::leaf(a), PaneNode::leaf(b)], vec![0.5, 0.5]);
    PaneNode::split(Axis::Row, vec![col("a", "b"), col("c", "d")], vec![0.3, 0.7])
}

#[test]
fn equalize_panes_follows_the_shape_preference() {
    for (shape, axis) in [(EqualizeShape::Rows, Axis::Column), (EqualizeShape::Columns, Axis::Row), (EqualizeShape::Auto, Axis::Row), (EqualizeShape::Keep, Axis::Row)] {
        let mut rig = rig(&["a", "b", "c", "d"]);
        rig.s.set_equalize_shape(shape);
        rig.s.set_panes(Some(columns_grid()), Some("a"));
        rig.s.equalize_panes();
        assert_eq!(rig.s.panes().unwrap().as_split().unwrap().axis, axis, "{shape:?}");
        assert_eq!(weights(&rig), [0.5, 0.5], "{shape:?}");
    }
}

#[test]
fn equalize_split_only_touches_the_split_at_that_path() {
    let mut rig = rig(&["a", "b", "c"]);
    let col = PaneNode::split(Axis::Column, vec![PaneNode::leaf("b"), PaneNode::leaf("c")], vec![0.1, 0.9]);
    rig.s.set_panes(Some(PaneNode::split(Axis::Row, vec![PaneNode::leaf("a"), col], vec![0.2, 0.8])), Some("a"));
    rig.s.equalize_split(&[1]);
    assert_eq!(weights(&rig), [0.2, 0.8]);
    assert_eq!(rig.s.panes().unwrap().as_split().unwrap().children[1].as_split().unwrap().weights, [0.5, 0.5]);
}

#[test]
fn zoom_shows_one_pane_without_touching_or_saving_the_tree() {
    let mut rig = rig(&["a", "b"]);
    rig.s.set_panes(Some(row(&["a", "b"])), Some("a"));
    rig.s.flush();
    rig.s.toggle_zoom_of("b");
    assert_eq!(rig.s.zoomed(), Some("b"));
    assert_eq!(rig.s.focused_pane_id(), Some("b"));
    assert_eq!(rig.s.panes(), Some(&row(&["a", "b"])));
    assert!(!rig.s.save_pending());
    rig.s.toggle_zoom_of("b");
    assert_eq!(rig.s.zoomed(), None);
}

#[test]
fn toggle_zoom_acts_on_the_focused_pane() {
    let mut rig = rig(&["a", "b"]);
    rig.s.set_panes(Some(row(&["a", "b"])), Some("a"));
    rig.s.toggle_zoom();
    assert_eq!(rig.s.zoomed(), Some("a"));
    rig.s.toggle_zoom();
    assert_eq!(rig.s.zoomed(), None);
}

#[test]
fn a_lone_pane_does_not_zoom() {
    let mut rig = rig(&["a"]);
    rig.s.look("a");
    rig.s.toggle_zoom();
    assert_eq!(rig.s.zoomed(), None);
}

#[test]
fn zoom_ends_when_the_zoomed_pane_leaves_the_tree() {
    let mut rig = rig(&["a", "b", "c"]);
    rig.s.set_panes(Some(row(&["a", "b", "c"])), Some("a"));
    rig.s.toggle_zoom_of("b");
    rig.s.dismiss("b");
    assert_eq!(rig.s.zoomed(), None);
    rig.s.drop_tab("b", "a", PaneDrop::Split(DropSide::Right));
    assert_eq!(rig.s.zoomed(), None);
}

#[test]
fn zoom_ends_when_the_zoomed_tab_closes() {
    let mut rig = rig(&["a", "b", "c"]);
    rig.s.set_panes(Some(row(&["a", "b", "c"])), Some("a"));
    rig.s.toggle_zoom_of("a");
    rig.s.close_tab("a");
    assert_eq!(rig.s.zoomed(), None);
}

#[test]
fn zoom_ends_when_another_pane_takes_focus() {
    let mut rig = rig(&["a", "b"]);
    rig.s.set_panes(Some(row(&["a", "b"])), Some("a"));
    rig.s.toggle_zoom_of("a");
    rig.s.cycle_pane(1);
    assert_eq!(rig.s.zoomed(), None);
}

#[test]
fn zoom_ends_when_a_replace_takes_the_zoomed_place() {
    let mut rig = rig(&["a", "b", "c"]);
    rig.s.set_panes(Some(row(&["a", "b"])), Some("a"));
    rig.s.toggle_zoom_of("a");
    rig.s.select("c");
    assert_eq!(rig.s.order(), ["c", "b"]);
    assert_eq!(rig.s.zoomed(), None);
}

#[test]
fn unzoom_reports_whether_there_was_a_zoom() {
    let mut rig = rig(&["a", "b"]);
    rig.s.set_panes(Some(row(&["a", "b"])), Some("a"));
    assert!(!rig.s.unzoom());
    rig.s.toggle_zoom();
    assert!(rig.s.unzoom());
    assert_eq!(rig.s.zoomed(), None);
}
