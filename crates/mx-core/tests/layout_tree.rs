use mx_core::layout::{self, Axis, DropSide, PaneNode};
use proptest::prelude::*;
use serde_json::json;

fn leaf(id: &str) -> PaneNode {
    PaneNode::leaf(id)
}

fn row(children: Vec<PaneNode>, weights: Vec<f64>) -> PaneNode {
    PaneNode::split(Axis::Row, children, weights)
}

fn column(children: Vec<PaneNode>, weights: Vec<f64>) -> PaneNode {
    PaneNode::split(Axis::Column, children, weights)
}

fn weights(n: &PaneNode) -> Vec<f64> {
    n.as_split().unwrap().weights.clone()
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

fn index_in(ids: &'static [&'static str]) -> impl Fn(&str) -> i64 {
    move |id| ids.iter().position(|x| *x == id).map_or(-1, |i| i as i64)
}

#[test]
fn inserting_right_of_a_leaf_makes_a_half_half_row() {
    let n = leaf("a").insert("b", "a", DropSide::Right);
    assert_eq!(n.order(), ["a", "b"]);
    assert_eq!(n.as_split().unwrap().axis, Axis::Row);
    assert!(close(&weights(&n), &[0.5, 0.5]));
}

#[test]
fn inserting_on_top_of_a_leaf_makes_a_column_with_the_new_one_first() {
    let n = leaf("a").insert("b", "a", DropSide::Top);
    assert_eq!(n.as_split().unwrap().axis, Axis::Column);
    assert_eq!(n.order(), ["b", "a"]);
}

#[test]
fn inserting_along_the_axis_halves_the_target_weight() {
    let n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]).insert("c", "b", DropSide::Right);
    assert_eq!(n.order(), ["a", "b", "c"]);
    assert!(close(&weights(&n), &[0.5, 0.25, 0.25]));
    assert!(
        n.as_split()
            .unwrap()
            .children
            .iter()
            .all(|c| matches!(c, PaneNode::Leaf(_)))
    );
}

#[test]
fn inserting_across_the_axis_wraps_the_target_in_a_split() {
    let n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]).insert("c", "b", DropSide::Bottom);
    let inner = n.as_split().unwrap().children[1].as_split().unwrap();
    assert_eq!(inner.axis, Axis::Column);
    assert_eq!(n.as_split().unwrap().children[1].order(), ["b", "c"]);
}

#[test]
fn inserting_next_to_a_missing_target_changes_nothing() {
    let n = leaf("a").insert("b", "zz", DropSide::Right);
    assert_eq!(n, leaf("a"));
    let r = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]);
    assert_eq!(r.clone().insert("c", "zz", DropSide::Left), r);
}

#[test]
fn removing_spreads_the_freed_weight_proportionally() {
    let n = row(vec![leaf("a"), leaf("b"), leaf("c")], vec![0.5, 0.25, 0.25])
        .remove("a")
        .unwrap();
    assert_eq!(n.order(), ["b", "c"]);
    assert!(close(&weights(&n), &[0.5, 0.5]));
}

#[test]
fn removing_one_of_two_collapses_to_the_other_leaf() {
    let n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5])
        .remove("b")
        .unwrap();
    assert_eq!(n, leaf("a"));
}

#[test]
fn removing_absorbs_a_same_axis_child() {
    let tree = row(
        vec![
            leaf("a"),
            column(
                vec![leaf("b"), row(vec![leaf("c"), leaf("d")], vec![0.5, 0.5])],
                vec![0.5, 0.5],
            ),
        ],
        vec![0.5, 0.5],
    );
    let n = tree.remove("b").unwrap();
    assert_eq!(n.as_split().unwrap().axis, Axis::Row);
    assert_eq!(n.order(), ["a", "c", "d"]);
    assert!(close(&weights(&n), &[0.5, 0.25, 0.25]));
}

#[test]
fn removing_the_only_leaf_leaves_nothing() {
    assert_eq!(leaf("a").remove("a"), None);
}

#[test]
fn removing_with_zero_weights_equalizes() {
    let n = row(vec![leaf("a"), leaf("b"), leaf("c")], vec![1.0, 0.0, 0.0])
        .remove("a")
        .unwrap();
    assert!(close(&weights(&n), &[0.5, 0.5]));
}

#[test]
fn resizing_moves_the_gutter_by_the_fraction_of_the_total() {
    let mut n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]);
    n.as_split_mut().unwrap().resize(0, 100.0, 1000.0);
    assert!(close(&weights(&n), &[0.6, 0.4]));
}

#[test]
fn resizing_stops_at_the_minimum_extent() {
    let mut n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]);
    n.as_split_mut().unwrap().resize(0, -10000.0, 1000.0);
    assert!((weights(&n)[0] - 0.24).abs() < 1e-9);
}

#[test]
fn resizing_ignores_bad_gutters_and_totals() {
    let mut n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]);
    let s = n.as_split_mut().unwrap();
    s.resize(-1, 100.0, 1000.0);
    s.resize(1, 100.0, 1000.0);
    s.resize(0, 100.0, 0.0);
    assert!(close(&s.weights, &[0.5, 0.5]));
}

#[test]
fn resizing_a_too_small_split_does_nothing() {
    let mut n = row(vec![leaf("a"), leaf("b"), leaf("c")], vec![0.4, 0.4, 0.2]);
    n.as_split_mut().unwrap().resize(0, 50.0, 300.0);
    assert!(close(&weights(&n), &[0.4, 0.4, 0.2]));
}

#[test]
fn edge_along_the_root_axis_scales_existing_weights() {
    let n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]).edge("c", DropSide::Left);
    assert_eq!(n.order(), ["c", "a", "b"]);
    assert!(close(&weights(&n), &[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]));
}

#[test]
fn edge_across_the_root_axis_wraps_the_root() {
    let n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]).edge("c", DropSide::Bottom);
    assert_eq!(n.as_split().unwrap().axis, Axis::Column);
    assert_eq!(n.order(), ["a", "b", "c"]);
    assert!(close(&weights(&n), &[0.5, 0.5]));
}

#[test]
fn swap_hands_the_leaf_another_tab() {
    let mut n = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]);
    n.swap("b", "c");
    n.swap("zz", "d");
    assert_eq!(n.order(), ["a", "c"]);
    assert!(n.has("c") && !n.has("b"));
    assert_eq!(n.count(), 2);
}

#[test]
fn same_shape_ignores_weights() {
    let a = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]);
    let b = row(vec![leaf("a"), leaf("b")], vec![0.7, 0.3]);
    let c = column(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]);
    assert!(layout::same_shape(Some(&a), Some(&b)));
    assert!(!layout::same_shape(Some(&a), Some(&c)));
    assert!(layout::same_shape(None, None));
    assert!(!layout::same_shape(Some(&a), None));
}

#[test]
fn json_round_trip_keeps_order_and_weights() {
    let tree = row(
        vec![
            leaf("a"),
            column(vec![leaf("b"), leaf("c")], vec![0.7, 0.3]),
        ],
        vec![0.4, 0.6],
    );
    let j = layout::to_json(Some(&tree), &index_in(&["a", "b", "c"])).unwrap();
    assert_eq!(
        j,
        json!({"axis": "row", "weights": [0.4, 0.6], "children": [0, {"axis": "column", "weights": [0.7, 0.3], "children": [1, 2]}]})
    );
    let ids = ["a", "b", "c"];
    let back = layout::from_json(&j, &|i| ids.get(i as usize).map(|s| s.to_string())).unwrap();
    assert_eq!(back.order(), ["a", "b", "c"]);
    assert!(close(&weights(&back), &[0.4, 0.6]));
    assert!(close(
        &back.as_split().unwrap().children[1]
            .as_split()
            .unwrap()
            .weights,
        &[0.7, 0.3]
    ));
}

#[test]
fn json_drops_unsaved_leaves_and_collapses() {
    let tree = row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]);
    let j = layout::to_json(Some(&tree), &index_in(&["a"])).unwrap();
    assert_eq!(j, json!(0));
    assert_eq!(layout::to_json(Some(&tree), &index_in(&[])), None);
}

#[test]
fn json_renormalizes_kept_weights() {
    let tree = row(vec![leaf("a"), leaf("b"), leaf("c")], vec![0.5, 0.25, 0.25]);
    let j = layout::to_json(Some(&tree), &index_in(&["b", "c"])).unwrap();
    assert_eq!(
        j,
        json!({"axis": "row", "weights": [0.5, 0.5], "children": [0, 1]})
    );
}

#[test]
fn from_json_defaults_missing_weights_and_unknown_axis() {
    let ids = ["a", "b", "c"];
    let at = |i: i64| ids.get(i as usize).map(|s| s.to_string());
    let n = layout::from_json(
        &json!({"axis": "diagonal", "children": [0, 1, 2], "weights": [2]}),
        &at,
    )
    .unwrap();
    assert_eq!(n.as_split().unwrap().axis, Axis::Row);
    assert!(close(&weights(&n), &[0.5, 0.25, 0.25]));
    assert_eq!(layout::from_json(&json!("x"), &at), None);
    assert_eq!(layout::from_json(&json!(1.0), &at), None);
    assert_eq!(
        layout::from_json(&json!({"children": [9, 1]}), &at),
        Some(leaf("b"))
    );
}

#[test]
fn legacy_two_slot_layout_is_a_row_of_two_or_one_leaf() {
    let two = layout::legacy_two_slot(Some("a".into()), Some("b".into())).unwrap();
    assert_eq!(two, row(vec![leaf("a"), leaf("b")], vec![0.5, 0.5]));
    assert_eq!(
        layout::legacy_two_slot(Some("a".into()), None),
        Some(leaf("a"))
    );
    assert_eq!(layout::legacy_two_slot(None, Some("b".into())), None);
}

#[derive(Debug, Clone)]
enum Op {
    Insert(usize, u8),
    Edge(u8),
    Remove(usize),
}

fn side(n: u8) -> DropSide {
    [
        DropSide::Left,
        DropSide::Right,
        DropSide::Top,
        DropSide::Bottom,
        DropSide::Center,
    ][n as usize % 5]
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(
        prop_oneof![
            (any::<usize>(), any::<u8>()).prop_map(|(t, s)| Op::Insert(t, s)),
            any::<u8>().prop_map(Op::Edge),
            any::<usize>().prop_map(Op::Remove),
        ],
        1..40,
    )
}

fn check_invariants(node: &PaneNode) -> Result<(), TestCaseError> {
    if let PaneNode::Split(s) = node {
        prop_assert!(s.children.len() >= 2, "single-child split");
        prop_assert_eq!(s.children.len(), s.weights.len());
        let sum: f64 = s.weights.iter().sum();
        prop_assert!((sum - 1.0).abs() < 1e-6, "weights sum {}", sum);
        prop_assert!(s.weights.iter().all(|w| *w > 0.0));
        for c in &s.children {
            check_invariants(c)?;
        }
    }
    Ok(())
}

proptest! {
    #[test]
    fn insert_and_remove_keep_the_tree_well_formed(script in ops()) {
        let mut root: Option<PaneNode> = None;
        let mut model: Vec<String> = Vec::new();
        let mut next = 0;
        for op in script {
            match op {
                Op::Insert(t, s) if !model.is_empty() => {
                    let id = format!("t{next}");
                    next += 1;
                    let target = model[t % model.len()].clone();
                    let side = side(s);
                    root = Some(root.take().unwrap().insert(&id, &target, side));
                    let at = model.iter().position(|x| *x == target).unwrap();
                    model.insert(if side.leading() { at } else { at + 1 }, id);
                }
                Op::Remove(t) if !model.is_empty() => {
                    let id = model.remove(t % model.len());
                    root = root.take().and_then(|r| r.remove(&id));
                }
                Op::Edge(s) | Op::Insert(_, s) => {
                    let id = format!("t{next}");
                    next += 1;
                    let side = side(s);
                    root = Some(match root.take() {
                        Some(r) => r.edge(&id, side),
                        None => PaneNode::leaf(id.clone()),
                    });
                    if side.leading() { model.insert(0, id) } else { model.push(id) }
                }
                Op::Remove(_) => {}
            }
            prop_assert_eq!(layout::order(root.as_ref()), model.clone());
            if let Some(r) = &root {
                check_invariants(r)?;
            }
        }
    }

    #[test]
    fn persistence_round_trip_preserves_order_and_shape_after_remove(script in ops()) {
        let mut root: Option<PaneNode> = None;
        let mut ids: Vec<String> = Vec::new();
        for (n, op) in script.into_iter().enumerate() {
            let id = format!("t{n}");
            root = Some(match (root.take(), op) {
                (Some(r), Op::Insert(t, s)) if !ids.is_empty() => r.insert(&id, &ids[t % ids.len()].clone(), side(s)),
                (Some(r), _) => r.edge(&id, side(n as u8)),
                (None, _) => PaneNode::leaf(id.clone()),
            });
            ids.push(id);
        }
        let victim = ids[ids.len() / 2].clone();
        let after = root.unwrap().remove(&victim);
        if let Some(r) = &after {
            prop_assert!(!r.has(&victim));
            check_invariants(r)?;
        }
        let order = layout::order(after.as_ref());
        let j = layout::to_json(after.as_ref(), &|id| order.iter().position(|x| x == id).map_or(-1, |i| i as i64));
        let back = j.and_then(|j| layout::from_json(&j, &|i| order.get(i as usize).cloned()));
        prop_assert_eq!(layout::order(back.as_ref()), order.clone());
        prop_assert!(layout::same_shape(back.as_ref(), after.as_ref()));
    }
}
