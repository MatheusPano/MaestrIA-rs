use mx_core::layout::{Axis, PaneNode};
use mx_core::model::{PaneGroup, TabKind};
use serde_json::json;

use super::Rig;
use crate::TabTint;

fn g3() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    for id in ["um", "dois", "tres"] {
        rig.shell(id);
    }
    let tree = PaneNode::split(
        Axis::Row,
        vec![
            PaneNode::leaf("um"),
            PaneNode::split(Axis::Column, vec![PaneNode::leaf("dois"), PaneNode::leaf("tres")], vec![0.5, 0.5]),
        ],
        vec![0.6, 0.4],
    );
    rig.s.set_panes(Some(tree), Some("um"));
    rig
}

fn fourth(rig: &mut Rig) {
    rig.add_tab("quatro", "/repo", TabKind::Shell, "/repo", Some("quatro"));
}

fn grouped() -> (Rig, String) {
    let mut rig = g3();
    let id = rig.s.group_panes().unwrap();
    (rig, id)
}

#[test]
fn g1_group_panes_names_by_summary_and_stamps_members() {
    let (rig, id) = grouped();
    let g = rig.s.group_by_id(&id).unwrap();
    assert_eq!((g.name.as_str(), g.count()), ("3 terminais", 3));
    assert_eq!(rig.s.groups().len(), 1);
    assert!(rig.s.tabs().iter().all(|t| t.group_id.as_deref() == Some(id.as_str())));
}

#[test]
fn g2_a_single_pane_is_not_a_group() {
    let mut rig = g3();
    rig.s.look("um");
    assert!(rig.s.group_panes().is_none());
    assert!(rig.s.groups().is_empty());
}

#[test]
fn g3_grouping_twice_numbers_the_name() {
    let mut rig = g3();
    let a = rig.s.group_panes().unwrap();
    let b = rig.s.group_panes().unwrap();
    let names: Vec<&str> = rig.s.groups().iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["3 terminais", "3 terminais (2)"]);
    assert_ne!(a, b);
}

#[test]
fn g4_ungroup_forgets_but_keeps_the_screen() {
    let (mut rig, id) = grouped();
    rig.s.ungroup(&id);
    assert!(rig.s.groups().is_empty());
    assert_eq!(rig.s.tabs().len(), 3);
    assert!(rig.s.tabs().iter().all(|t| t.group_id.is_none()));
    assert_eq!(rig.s.pane_count(), 3);
}

#[test]
fn g5_saving_again_restamps_only_the_members() {
    let (mut rig, id) = grouped();
    rig.s.dismiss("tres");
    let name = rig.s.group_by_id(&id).unwrap().name.clone();
    rig.s.save_group(&name);
    assert!(rig.t("tres").group_id.is_none());
    assert_eq!(rig.t("um").group_id.as_deref(), Some(id.as_str()));
    assert_eq!(rig.s.group_by_id(&id).unwrap().count(), 2);
}

#[test]
fn g6_g7_members_share_one_group_colour_that_survives_json() {
    let (mut rig, id) = grouped();
    fourth(&mut rig);
    let tint = rig.s.tint_of("um");
    assert!(matches!(tint, Some(TabTint::Group(i)) if i < 4));
    assert_eq!(rig.s.tint_of("dois"), tint);
    assert_eq!(rig.s.tint_of("quatro"), None);
    let g = rig.s.group_by_id(&id).unwrap();
    let text = g.to_json().to_string();
    let back = PaneGroup::from_json(&serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(back.tint_index(4), g.tint_index(4));
}

#[test]
fn g8_swapping_a_member_out_leaves_no_active_group() {
    let (mut rig, id) = grouped();
    fourth(&mut rig);
    rig.s.panes_mut().unwrap().swap("tres", "quatro");
    assert!(rig.s.active_group().is_none());
    assert!(!rig.s.showing(&id));
}

#[test]
fn g9_selecting_a_stranger_replaces_the_grouped_grid() {
    let (mut rig, id) = grouped();
    fourth(&mut rig);
    rig.s.select("quatro");
    assert_eq!(rig.s.order(), ["quatro"]);
    assert_eq!(rig.s.focused_pane_id(), Some("quatro"));
    assert_eq!(rig.s.tabs().len(), 4);
    assert!(rig.s.group_by_id(&id).is_some());
    assert!(rig.s.active_group().is_none());
}

#[test]
fn g10_selecting_a_member_keeps_the_grid() {
    let (mut rig, _) = grouped();
    rig.s.select("tres");
    assert_eq!(rig.s.pane_count(), 3);
}

#[test]
fn g11_without_a_group_select_replaces_the_focused_pane() {
    let mut rig = g3();
    fourth(&mut rig);
    rig.s.select("quatro");
    assert_eq!(rig.s.order(), ["quatro", "dois", "tres"]);
}

#[test]
fn g12_save_group_keeps_recipes_and_tree() {
    let mut rig = g3();
    rig.tm("um").hooks.touched.push("/repo/a.dart".into());
    rig.tm("um").done = true;
    let id = rig.s.save_group("grid de 3").unwrap();
    let g = rig.s.group_by_id(&id).unwrap();
    assert_eq!((g.count(), g.summary().as_str(), g.where_().as_str()), (3, "3 terminais", "repo"));
    let labels: Vec<&str> = g.panes.iter().map(|p| p["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["um", "dois", "tres"]);
    for p in &g.panes {
        assert!(p.get("touched").is_none() && p.get("done").is_none() && p.get("group").is_none());
    }
    let tree = g.tree.as_ref().unwrap();
    assert_eq!(tree["axis"], "row");
    assert_eq!(tree["children"][0], 0);
    assert_eq!(tree["children"][1]["axis"], "column");
    assert_eq!(tree["children"][1]["children"], json!([1, 2]));
}

#[test]
fn g13_groups_persist_and_reopen() {
    let (mut rig, id) = grouped();
    let j = rig.written();
    let groups = j["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    let back = PaneGroup::from_json(&groups[0]).unwrap();
    let g = rig.s.group_by_id(&id).unwrap();
    assert_eq!((&back.id, &back.name, back.count(), &back.tree), (&g.id, &g.name, g.count(), &g.tree));
    for p in j["layout"]["panes"].as_array().unwrap() {
        assert_eq!(p["group"], json!(id));
    }
    rig.s.set_panes(None, None);
    rig.s.open_group(&id);
    assert_eq!(rig.s.order(), ["um", "dois", "tres"]);
}

#[test]
fn g14_open_group_restores_the_exact_grid() {
    let (mut rig, id) = grouped();
    fourth(&mut rig);
    rig.s.select("quatro");
    rig.s.open_group(&id);
    assert_eq!(rig.s.order(), ["um", "dois", "tres"]);
    assert_eq!(rig.s.tabs().len(), 4);
    let root = rig.s.panes().unwrap().as_split().unwrap();
    assert_eq!(root.axis, Axis::Row);
    assert!((root.weights[0] - 0.6).abs() < 1e-9);
    assert_eq!(root.children[1].order(), ["dois", "tres"]);
    assert_eq!(rig.s.focused_pane_id(), Some("um"));
    assert!(rig.s.showing(&id));
}

#[test]
fn g15_saving_under_the_same_name_any_case_replaces() {
    let mut rig = g3();
    let id = rig.s.save_group("grid").unwrap();
    rig.s.dismiss("tres");
    let again = rig.s.save_group("GRID").unwrap();
    assert_eq!(rig.s.groups().len(), 1);
    assert_eq!(again, id);
    let g = rig.s.group_by_id(&id).unwrap();
    assert_eq!((g.count(), g.name.as_str()), (2, "GRID"));
}

#[test]
fn g16_nothing_on_screen_saves_nothing() {
    let mut rig = g3();
    rig.s.set_panes(None, None);
    assert!(rig.s.save_group("vazio").is_none());
    assert!(rig.s.save_group("").is_none());
    assert!(rig.s.group_panes().is_none());
    assert!(rig.s.groups().is_empty());
}

#[test]
fn g17_remove_group_keeps_the_panels() {
    let mut rig = g3();
    let id = rig.s.save_group("grid").unwrap();
    rig.s.remove_group(&id);
    assert!(rig.s.groups().is_empty());
    assert_eq!((rig.s.tabs().len(), rig.s.pane_count()), (3, 3));
}

#[test]
fn g22_groups_collapsed_is_saved() {
    let mut rig = g3();
    rig.s.save_group("grid");
    rig.s.toggle_groups_collapsed();
    assert_eq!(rig.written()["groupsCollapsed"], true);
}

#[test]
fn g23_clear_groups_forgets_all_and_counts() {
    let mut rig = g3();
    rig.s.group_panes();
    rig.s.save_group("grid da tarde");
    let order = rig.s.order();
    assert_eq!(rig.s.clear_groups(), 2);
    assert!(rig.s.groups().is_empty());
    assert_eq!(rig.s.tabs().len(), 3);
    assert_eq!(rig.s.order(), order);
    assert!(rig.s.tabs().iter().all(|t| t.group_id.is_none()));
    assert_eq!(rig.s.clear_groups(), 0);
}
