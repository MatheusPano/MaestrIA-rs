use mx_core::model::sidebar::RowKey;

use super::Rig;
use crate::RowPlace;

fn tabs(ids: &[&str]) -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    for id in ids {
        rig.panel(id);
    }
    rig
}

#[test]
fn r1_moving_down_lands_after_the_target() {
    let mut rig = tabs(&["um", "dois", "tres", "quatro"]);
    rig.s.move_tab("um", "tres");
    assert_eq!(rig.ids(), ["dois", "tres", "um", "quatro"]);
}

#[test]
fn r2_moving_up_lands_before_the_target() {
    let mut rig = tabs(&["um", "dois", "tres", "quatro"]);
    rig.s.move_tab("quatro", "dois");
    assert_eq!(rig.ids(), ["um", "quatro", "dois", "tres"]);
}

#[test]
fn r3_r4_moving_onto_itself_or_a_ghost_does_nothing() {
    let mut rig = tabs(&["um", "dois", "tres"]);
    rig.s.move_tab("dois", "dois");
    rig.s.move_tab("ghost", "um");
    assert_eq!(rig.ids(), ["um", "dois", "tres"]);
}

fn folders(names: &[&str]) -> Rig {
    let mut rig = Rig::new();
    for n in names {
        rig.folder(&format!("/{n}"), n);
    }
    rig
}

fn section(rig: &mut Rig, members: &[&str]) -> String {
    section_named(rig, "cefis", members)
}

fn section_named(rig: &mut Rig, name: &str, members: &[&str]) -> String {
    let roots: Vec<String> = members.iter().map(|m| format!("/{m}")).collect();
    let refs: Vec<&str> = roots.iter().map(String::as_str).collect();
    rig.s.create_workspace(name, &refs, None).unwrap()
}

fn root(name: &str) -> RowPlace {
    RowPlace::root(RowKey::Folder(format!("/{name}")))
}

fn ws_row(id: &str) -> RowPlace {
    RowPlace::root(RowKey::Workspace(id.to_owned()))
}

fn inside(name: &str, ws: &str) -> RowPlace {
    RowPlace::inside(&format!("/{name}"), ws)
}

fn rows(rig: &Rig) -> Vec<String> {
    rig.s
        .sidebar_rows()
        .into_iter()
        .map(|r| match r {
            RowKey::Workspace(id) => format!("ws:{}", rig.s.workspace_by_id(&id).unwrap().name),
            RowKey::Folder(root) => root.trim_start_matches('/').to_owned(),
        })
        .collect()
}

fn members(rig: &Rig, ws: &str) -> Vec<String> {
    super::names(&rig.s.folders_of(ws))
}

#[test]
fn r6_r7_root_folders_slide_into_the_target_slot() {
    let mut rig = folders(&["um", "dois", "tres", "quatro"]);
    rig.s.drop_row(&root("quatro"), &root("dois"), false);
    assert_eq!(rows(&rig), ["um", "quatro", "dois", "tres"]);
    let mut rig = folders(&["um", "dois", "tres", "quatro"]);
    rig.s.drop_row(&root("um"), &root("tres"), false);
    assert_eq!(rows(&rig), ["dois", "tres", "um", "quatro"]);
}

#[test]
fn r8_dropping_a_row_on_itself_is_refused() {
    let mut rig = folders(&["um", "dois"]);
    assert!(!rig.s.can_drop(&root("dois"), &root("dois"), false));
    rig.s.drop_row(&root("dois"), &root("dois"), false);
    assert_eq!(rows(&rig), ["um", "dois"]);
}

#[test]
fn r9_a_workspace_moves_as_one_row() {
    let mut rig = folders(&["um", "api", "web", "dois"]);
    let ws = section(&mut rig, &["api", "web"]);
    rig.s.drop_row(&ws_row(&ws), &root("um"), false);
    assert_eq!(rows(&rig), ["ws:cefis", "um", "dois"]);
    assert_eq!(members(&rig, &ws), ["api", "web"]);
}

#[test]
fn r10_a_folder_dropped_on_a_workspace_edge_only_reorders() {
    let mut rig = folders(&["um", "dois", "api"]);
    let ws = section(&mut rig, &["api"]);
    rig.s.drop_row(&root("um"), &ws_row(&ws), false);
    assert_eq!(rows(&rig), ["dois", "ws:cefis", "um"]);
}

#[test]
fn r11_workspaces_do_not_nest() {
    let mut rig = folders(&["api", "web"]);
    let a = section_named(&mut rig, "a", &["api"]);
    let b = section_named(&mut rig, "b", &["web"]);
    assert!(!rig.s.can_drop(&ws_row(&a), &inside("web", &b), false));
    assert!(!rig.s.can_drop(&ws_row(&a), &ws_row(&b), true));
}

#[test]
fn r12_reorder_inside_a_workspace() {
    let mut rig = folders(&["api", "web", "app"]);
    let ws = section(&mut rig, &["api", "web", "app"]);
    rig.s.drop_row(&inside("app", &ws), &inside("api", &ws), false);
    assert_eq!(members(&rig, &ws), ["app", "api", "web"]);
}

#[test]
fn r13_dropping_out_to_the_root_lands_on_that_row() {
    let mut rig = folders(&["api", "web", "solta"]);
    let ws = section(&mut rig, &["api", "web"]);
    rig.s.drop_row(&inside("api", &ws), &root("solta"), false);
    assert_eq!(members(&rig, &ws), ["web"]);
    assert_eq!(rows(&rig), ["api", "solta", "ws:cefis"]);
}

#[test]
fn r14_a_root_folder_dropped_inside_joins_at_that_slot() {
    let mut rig = folders(&["api", "web", "solta"]);
    let ws = section(&mut rig, &["api", "web"]);
    rig.s.drop_row(&root("solta"), &inside("web", &ws), false);
    assert_eq!(members(&rig, &ws), ["api", "solta", "web"]);
    assert_eq!(rows(&rig), ["ws:cefis"]);
}

#[test]
fn r15_into_another_workspace_header_moves_it() {
    let mut rig = folders(&["api", "web"]);
    let a = section_named(&mut rig, "a", &["api"]);
    let b = section_named(&mut rig, "b", &["web"]);
    rig.s.drop_row(&inside("api", &a), &ws_row(&b), true);
    assert!(members(&rig, &a).is_empty());
    assert_eq!(members(&rig, &b), ["web", "api"]);
}

#[test]
fn r16_a_root_folder_into_a_header_joins_it() {
    let mut rig = folders(&["api", "solta"]);
    let ws = section(&mut rig, &["api"]);
    rig.s.drop_row(&root("solta"), &ws_row(&ws), true);
    assert_eq!(members(&rig, &ws), ["api", "solta"]);
    assert_eq!(rows(&rig), ["ws:cefis"]);
}

#[test]
fn r17_into_its_own_workspace_is_refused() {
    let mut rig = folders(&["api"]);
    let ws = section(&mut rig, &["api"]);
    assert!(!rig.s.can_drop(&inside("api", &ws), &ws_row(&ws), true));
}

#[test]
fn r18_a_root_folder_on_a_header_edge_reorders() {
    let mut rig = folders(&["solta", "api"]);
    let ws = section(&mut rig, &["api"]);
    rig.s.drop_row(&root("solta"), &ws_row(&ws), false);
    assert_eq!(members(&rig, &ws), ["api"]);
    assert_eq!(rows(&rig), ["ws:cefis", "solta"]);
}
