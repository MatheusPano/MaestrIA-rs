use mx_core::layout::DropSide;
use mx_core::model::TabKind;
use mx_plugins::Plugins;
use mx_plugins::rpc::ViewOpen;
use serde_json::{Value, json};

use super::Rig;

fn write_plugin(root: &str, id: &str, manifest: Value) {
    let dir = std::path::Path::new(root).join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("maestria-plugin.json"), manifest.to_string()).unwrap();
}

fn board(screen: Value) -> Value {
    let mut contributes = json!({"commands": [{"id": "quadro", "title": "wiboor: o quadro"}]});
    if !screen.is_null() {
        contributes["screen"] = screen;
    }
    json!({"id": "wiboor", "name": "Wiboor", "version": "1", "icon": "terminal", "main": ["true"], "contributes": contributes})
}

fn git() -> Value {
    json!({"id": "git", "name": "git", "version": "1", "icon": "terminal",
        "contributes": {"commands": [{"id": "status", "title": "git: status", "run": "true"}]}})
}

fn desk(screen: Value) -> Rig {
    let mut rig = Rig::new();
    let root = rig.mkdir("plugins");
    let data = rig.mkdir("plugin-data");
    write_plugin(&root, "wiboor", board(screen));
    write_plugin(&root, "git", git());
    rig.s.attach_plugins(Plugins::with_dirs(root.into(), data.into()));
    rig.s1();
    rig.panel("left");
    rig.panel("right");
    rig.s.select("left");
    rig.s.drop_tab("right", "left", DropSide::Right);
    rig.s.focus_pane("right");
    rig
}

fn open(rig: &mut Rig, view: &str) -> String {
    let p = rig.s.plugins().unwrap().get("wiboor").unwrap();
    let req = ViewOpen { view_id: view.into(), title: view.into(), ..ViewOpen::default() };
    rig.s.view_open(&p, req)
}

fn panes(rig: &Rig) -> Vec<String> {
    rig.s.order().iter().map(|id| rig.s.title_of(id)).collect()
}

#[test]
fn owned_terminals_live_on_the_plugin_screen_beside_its_window() {
    let mut rig = desk(json!(true));
    open(&mut rig, "hosts");
    rig.add_tab("ssh-1", "/repo", TabKind::Shell, "/repo", Some("ssh-1"));
    rig.add_tab("ssh-2", "/repo", TabKind::Shell, "/repo", Some("ssh-2"));
    rig.tm("ssh-1").owner = Some("wiboor".into());
    rig.tm("ssh-2").owner = Some("wiboor".into());
    rig.s.select("ssh-1");
    assert_eq!(rig.s.screen(), Some("wiboor"));
    assert_eq!(rig.s.shown_plugin().as_deref(), Some("wiboor"));
    assert_eq!(panes(&rig), ["hosts", "ssh-1"]);
    let grid = rig.s.order()[0].clone();
    rig.s.focus_pane(&grid);
    rig.s.select("ssh-2");
    assert_eq!(panes(&rig), ["hosts", "ssh-2"]);
    rig.s.show_sidebar_view(None);
    assert_eq!(panes(&rig), ["left", "right"]);
}

#[test]
fn a_released_window_goes_to_the_sessions_and_returns_home() {
    let mut rig = desk(json!(true));
    open(&mut rig, "quadro");
    let tarefa = open(&mut rig, "tarefa-1");
    rig.s.release_view(&tarefa);
    assert_eq!(rig.s.screen(), None);
    assert_eq!(rig.s.shown_plugin(), None);
    assert_eq!(panes(&rig), ["left", "right", "tarefa-1"]);
    assert_eq!(rig.s.focused_tab_id().as_deref(), Some(tarefa.as_str()));
    assert_eq!(rig.s.screen_of(rig.t(&tarefa)), None);
    rig.s.show_sidebar_view(Some("wiboor"));
    assert_eq!(panes(&rig), ["quadro"]);
    open(&mut rig, "tarefa-1");
    assert_eq!(rig.s.screen(), None);
    assert_eq!(panes(&rig), ["left", "right", "tarefa-1"]);
    rig.s.return_view(&tarefa);
    assert_eq!(rig.s.screen(), Some("wiboor"));
    assert_eq!(panes(&rig), ["quadro", "tarefa-1"]);
    rig.s.show_sidebar_view(None);
    assert_eq!(panes(&rig), ["left", "right"]);
}

#[test]
fn the_plugin_screen_keeps_its_own_tree_and_focus() {
    let mut rig = desk(json!(true));
    let sessions = rig.s.panes().cloned();
    open(&mut rig, "quadro");
    assert_eq!(rig.s.screen(), Some("wiboor"));
    assert_eq!(rig.s.shown_plugin().as_deref(), Some("wiboor"));
    assert_eq!(panes(&rig), ["quadro"]);
    assert!(!rig.s.is_open("right"));
    let tarefa = open(&mut rig, "tarefa-1");
    assert_eq!(panes(&rig), ["quadro", "tarefa-1"]);
    rig.s.show_sidebar_view(None);
    assert_eq!(rig.s.screen(), None);
    assert_eq!(rig.s.panes().cloned(), sessions);
    assert_eq!(rig.s.focused_pane_id(), Some("right"));
    rig.s.show_sidebar_view(Some("wiboor"));
    assert_eq!(panes(&rig), ["quadro", "tarefa-1"]);
    assert_eq!(rig.s.focused_pane_id(), Some(tarefa.as_str()));
}

#[test]
fn selecting_a_session_brings_back_the_sessions_screen() {
    let mut rig = desk(json!(true));
    open(&mut rig, "quadro");
    rig.s.select("left");
    assert_eq!(rig.s.screen(), None);
    assert_eq!(rig.s.shown_plugin(), None);
    assert_eq!(panes(&rig), ["left", "right"]);
    assert_eq!(rig.s.focused_pane_id(), Some("left"));
}

#[test]
fn a_plugin_without_screen_shows_its_tab_over_the_sessions() {
    let mut rig = desk(json!(true));
    open(&mut rig, "quadro");
    rig.s.show_sidebar_view(Some("git"));
    assert_eq!(rig.s.screen(), None);
    assert_eq!(rig.s.shown_plugin().as_deref(), Some("git"));
    assert_eq!(panes(&rig), ["left", "right"]);
    open(&mut rig, "quadro");
    assert_eq!(rig.s.screen(), Some("wiboor"));
    assert_eq!(rig.s.shown_plugin().as_deref(), Some("wiboor"));
    rig.s.select("right");
    assert_eq!(rig.s.shown_plugin(), None);
}

#[test]
fn cycling_stays_inside_the_screen_in_view() {
    let mut rig = desk(json!(true));
    let quadro = open(&mut rig, "quadro");
    let tarefa = open(&mut rig, "tarefa");
    rig.s.cycle(1);
    assert_eq!(rig.s.focused_tab_id(), Some(quadro));
    rig.s.select("right");
    rig.s.cycle(1);
    assert_eq!(rig.s.screen(), None);
    assert_eq!(rig.s.focused_tab_id().as_deref(), Some("left"));
    assert!(!rig.s.is_open(&tarefa));
}

#[test]
fn without_screen_the_window_joins_the_sessions_grid() {
    let mut rig = desk(Value::Null);
    open(&mut rig, "quadro");
    assert_eq!(rig.s.screen(), None);
    assert_eq!(panes(&rig), ["left", "right", "quadro"]);
}

#[test]
fn closing_a_session_while_parked_leaves_no_hole() {
    let mut rig = desk(json!(true));
    open(&mut rig, "quadro");
    rig.s.close_tab("right");
    assert_eq!(panes(&rig), ["quadro"]);
    rig.s.show_sidebar_view(None);
    assert_eq!(panes(&rig), ["left"]);
    assert_eq!(rig.s.focused_tab_id().as_deref(), Some("left"));
}

#[test]
fn closing_the_last_window_leaves_the_screen_empty() {
    let mut rig = desk(json!(true));
    let quadro = open(&mut rig, "quadro");
    rig.s.close_tab(&quadro);
    assert_eq!(rig.s.screen(), Some("wiboor"));
    assert!(rig.s.panes().is_none());
    rig.s.show_sidebar_view(None);
    assert_eq!(panes(&rig), ["left", "right"]);
}

#[test]
fn pinned_sessions_stay_pinned_while_parked() {
    let mut rig = desk(json!(true));
    rig.s.toggle_pin("left");
    open(&mut rig, "quadro");
    rig.s.show_sidebar_view(Some("wiboor"));
    rig.s.show_sidebar_view(None);
    assert!(rig.t("left").pinned);
    assert!(rig.s.is_pinned("left"));
}

#[tokio::test]
async fn disabling_the_plugin_returns_to_the_sessions() {
    let mut rig = desk(json!(true));
    open(&mut rig, "quadro");
    rig.s.set_plugin_enabled("wiboor", false);
    rig.s.settle().await;
    assert_eq!(rig.s.screen(), None);
    assert_eq!(rig.s.shown_plugin(), None);
    assert_eq!(panes(&rig), ["left", "right"]);
}

#[test]
fn the_config_keeps_the_sessions_grid_and_the_plugin_tab() {
    let mut rig = desk(json!(true));
    open(&mut rig, "quadro");
    rig.s.show_sidebar_view(Some("wiboor"));
    let written = rig.written();
    let labels: Vec<&str> = written["layout"]["panes"].as_array().unwrap().iter().map(|p| p["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["left", "right"]);
    assert!(!written["layout"]["tree"].is_null());
    assert_eq!(written["layout"]["focused"], 1);
    assert_eq!(written["sidebarView"], "wiboor");
}

#[test]
fn clicking_the_current_icon_hides_the_sidebar() {
    let mut rig = desk(json!(true));
    rig.s.show_sidebar_view(None);
    assert!(rig.s.prefs().sidebar_hidden);
    rig.s.show_sidebar_view(Some("wiboor"));
    assert!(!rig.s.prefs().sidebar_hidden);
    assert_eq!(rig.s.screen(), Some("wiboor"));
}
