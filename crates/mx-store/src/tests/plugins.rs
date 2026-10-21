use std::sync::Arc;

use mx_core::model::{SuggestionChoice, TabKind};
use mx_plugins::rpc::{
    BadgeUpdate, FloatContent, OpenShell, PluginHostApi, SendText, SidebarUpdate, SuggestionCreated, ViewOpen,
    ViewUpdate,
};
use mx_plugins::{PluginInfo, PluginManifest, PluginState};
use serde_json::json;
use tokio::sync::oneshot;

use super::Rig;
use crate::{DialogAnswer, Effect};

fn info(contributes: serde_json::Value) -> PluginInfo {
    let m = json!({"id": "demo", "name": "Demo", "version": "1", "main": ["node", "main.js"], "contributes": contributes});
    let manifest = PluginManifest::parse(m.as_object().unwrap()).unwrap();
    PluginInfo {
        id: "demo".into(),
        dir: "/p/demo".into(),
        data_dir: "/d/demo".into(),
        manifest: Some(Arc::new(manifest)),
        problem: None,
        linked: false,
        enabled: true,
        crash: None,
        state: PluginState::Idle,
        palettes: Vec::new(),
    }
}

fn open(view: &str, blocks: serde_json::Value, modal: bool) -> ViewOpen {
    let blocks = blocks.as_array().unwrap().iter().filter_map(|b| b.as_object().cloned()).collect();
    ViewOpen { view_id: view.into(), title: "Painel".into(), blocks, rfw: None, modal }
}

fn rig() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("s");
    rig.s.look("s");
    rig
}

#[test]
fn view_open_places_beside_and_reopens_the_same_tab() {
    let mut rig = rig();
    let p = info(json!({}));
    let a = rig.s.view_open(&p, open("main", json!([]), false));
    assert_eq!(rig.s.order(), ["s".to_owned(), a.clone()]);
    assert!(rig.t(&a).is_plugin_view());
    assert_eq!(rig.s.subtitle(rig.t(&a)), "Plugin · Demo");
    rig.s.dismiss(&a);
    let b = rig.s.view_open(&p, open("main", json!([]), false));
    assert_eq!(a, b);
    assert!(rig.s.is_open(&a));
    assert!(rig.s.config().layout.unwrap().panes.len() == 1);
}

#[test]
fn view_update_reports_closed_views() {
    let mut rig = rig();
    let p = info(json!({}));
    let id = rig.s.view_open(&p, open("main", json!([]), false));
    let upd = |t: &str| ViewUpdate { view_id: "main".into(), title: Some(t.into()), blocks: None, rfw: None };
    assert!(rig.s.view_update("demo", upd("novo")));
    assert_eq!(rig.s.title(rig.t(&id)), "novo");
    rig.s.close_tab(&id);
    assert!(!rig.s.view_update("demo", upd("x")));
}

#[test]
fn modal_views_live_outside_the_grid_until_docked() {
    let mut rig = rig();
    let p = info(json!({}));
    let id = rig.s.view_open(&p, open("m", json!([]), true));
    assert!(!rig.s.has_tab(&id));
    assert_eq!(rig.s.modal().unwrap().0.id, id);
    assert_eq!(rig.s.session_list().len(), 2);
    rig.s.dock_modal();
    assert!(rig.s.modal().is_none());
    assert!(rig.s.is_open(&id));
    assert_eq!(rig.s.focused_pane_id(), Some(id.as_str()));
}

#[test]
fn closing_a_view_closes_its_embedded_terminals() {
    let mut rig = rig();
    let p = info(json!({}));
    let req = OpenShell { cwd: Some("/repo".into()), command: Some("top".into()), label: None, owned: true, tag: Some("t".into()), embedded: true };
    let term = rig.s.plugin_open_shell("demo", req);
    assert!(!rig.s.is_open(&term));
    assert_eq!(rig.t(&term).owner.as_deref(), Some("demo"));
    let view = rig.s.view_open(&p, open("main", json!([{"type": "terminal", "tabId": term}]), false));
    rig.s.view_close("demo", "main");
    assert!(!rig.s.has_tab(&view));
    assert!(!rig.s.has_tab(&term));
}

#[test]
fn consoles_take_lines_through_append_and_clear() {
    let mut rig = rig();
    let p = info(json!({}));
    let id = rig.s.view_open(&p, open("main", json!([{"type": "console", "id": "log"}]), false));
    assert!(rig.s.view_lines("demo", "main", "log", Some(&json!(["a", "b"]))));
    assert_eq!(rig.s.plugin_view(&id).unwrap().consoles["log"].texts(), ["a", "b"]);
    rig.s.view_lines("demo", "main", "log", None);
    assert!(rig.s.plugin_view(&id).unwrap().consoles["log"].texts().is_empty());
    assert!(!rig.s.view_lines("demo", "outra", "log", Some(&json!("x"))));
}

#[test]
fn sidebar_update_sets_badges_and_reports_visibility() {
    let mut rig = rig();
    let p = info(json!({"sidebar": true}));
    let req = |badge| SidebarUpdate { blocks: Some(Vec::new()), rfw: None, badge };
    assert!(!rig.s.sidebar_update(&p, req(BadgeUpdate::Set("3".into()))));
    assert_eq!(rig.s.plugin_ui().badges["demo"], "3");
    assert!(rig.s.sidebar_view_of("demo").is_some());
    rig.s.set_sidebar_view(Some("demo"));
    assert!(rig.s.sidebar_update(&p, req(BadgeUpdate::Clear)));
    assert!(!rig.s.plugin_ui().badges.contains_key("demo"));
}

#[test]
fn floats_show_update_and_hide() {
    let mut rig = rig();
    let p = info(json!({}));
    let c = FloatContent { id: "f".into(), width: Some(1000.0), height: None, corner: None, blocks: None, rfw: None };
    assert!(!rig.s.float_update("demo", &c));
    rig.s.float_show(&p, &c);
    assert_eq!(rig.s.plugin_ui().floats.shown()[0].width, 480.0);
    assert!(rig.s.float_update("demo", &c));
    rig.s.float_hide("demo", "f");
    assert!(rig.s.plugin_ui().floats.shown().is_empty());
}

#[test]
fn send_text_targets_live_sessions_only() {
    let mut rig = Rig::new();
    let none = rig.s.plugin_send_text(SendText { text: "x".into(), submit: true, tab_id: None });
    assert_eq!(none.unwrap_err().message, "nenhuma sessão na tela");
    rig.s1();
    rig.panel("s");
    rig.s.look("s");
    rig.s.plugin_send_text(SendText { text: "oi".into(), submit: true, tab_id: None }).unwrap();
    assert_eq!(rig.terms.log("s").submitted, ["oi"]);
    rig.s.plugin_send_text(SendText { text: "colado".into(), submit: false, tab_id: Some("s".into()) }).unwrap();
    assert_eq!(rig.s.take_effects(), [Effect::Paste { tab_id: "s".into(), text: "colado".into() }]);
    rig.tm("s").hibernated = true;
    let dead = rig.s.plugin_send_text(SendText { text: "x".into(), submit: true, tab_id: Some("s".into()) });
    assert_eq!(dead.unwrap_err().message, "\"s\" não tem processo pra receber texto");
    let ghost = rig.s.plugin_send_text(SendText { text: "x".into(), submit: true, tab_id: Some("g".into()) });
    assert!(ghost.is_err());
}

#[test]
fn session_json_reports_the_panel() {
    let mut rig = rig();
    rig.tm("s").session_id = Some("sid".into());
    let j = rig.s.session_list()[0].to_json("demo", true);
    assert_eq!(j["id"], "s");
    assert_eq!(j["folder"], "/repo");
    assert_eq!(j["sessionId"], "sid");
    assert_eq!(j["onScreen"], true);
    assert_eq!(j["focused"], true);
    assert!(j.get("activity").is_some());
    assert_eq!(rig.s.folder_list()[0].to_json()["name"], "meu-repo");
}

#[test]
fn tab_owner_resolves_focus_and_unknown_ids() {
    let rig = rig();
    assert_eq!(rig.s.plugin_tab_owner(None).unwrap().tab_id, "s");
    assert_eq!(rig.s.plugin_tab_owner(Some("g".into())).unwrap_err().message, "não há painel \"g\"");
}

#[tokio::test]
async fn host_calls_run_on_the_store_through_the_channel() {
    let mut rig = rig();
    let host = rig.s.plugin_host();
    let p = info(json!({}));
    let call = tokio::spawn(async move { host.show_banner(&p, "Demo: oi".into(), false).await });
    while !call.is_finished() {
        rig.s.pump();
        tokio::task::yield_now().await;
    }
    call.await.unwrap().unwrap();
    assert_eq!(rig.s.banner(), Some("Demo: oi"));
}

#[tokio::test]
async fn dialogs_wait_for_the_user() {
    let mut rig = rig();
    let host = rig.s.plugin_host();
    let p = info(json!({}));
    let req = mx_plugins::ui::dialogs::InputRequest::parse(json!({"title": "Nome"}).as_object().unwrap(), "Demo");
    let call = tokio::spawn(async move { host.input(&p, req).await });
    while rig.s.plugin_dialog().is_none() {
        rig.s.pump();
        tokio::task::yield_now().await;
    }
    rig.s.answer_dialog(DialogAnswer::Text(Some("lucas".into())));
    assert_eq!(call.await.unwrap().unwrap(), Some("lucas".into()));
}

#[test]
fn suggestion_created_runs_the_tracked_choice() {
    let mut rig = rig();
    let q = rig.s.suggest(mx_claude::mcp::NewSuggestion {
        tab_id: "s".into(),
        title: "t".into(),
        tldr: "".into(),
        prompt: "o pedido".into(),
    });
    let task = q.unwrap().task.id;
    let req = |task: &str| SuggestionCreated {
        suggestion_id: task.into(),
        reference: "TASK#1".into(),
        intro: "Vamos trabalhar na TASK#1".into(),
        branch: None,
        dir: None,
    };
    let (tx, mut rx) = oneshot::channel();
    rig.s.plugin_suggestion_created("demo", req(&task), tx);
    assert_eq!(rx.try_recv().unwrap().unwrap(), None);
    rig.s.track_suggestion(&task, "demo", "s", SuggestionChoice::Here);
    let (tx, mut rx) = oneshot::channel();
    rig.s.plugin_suggestion_created("demo", req(&task), tx);
    let created = rx.try_recv().unwrap().unwrap().unwrap();
    assert_eq!((created.tab_id.as_str(), created.cwd.as_str()), ("s", "/repo"));
    assert_eq!(rig.terms.log("s").submitted, ["Vamos trabalhar na TASK#1\n\no pedido"]);
}

#[test]
fn owned_shells_without_embedding_take_a_pane() {
    let mut rig = rig();
    let req = OpenShell { cwd: None, command: None, label: Some("tail".into()), owned: true, tag: None, embedded: false };
    let id = rig.s.plugin_open_shell("demo", req);
    assert!(rig.s.is_open(&id));
    assert_eq!(rig.t(&id).kind, TabKind::Shell);
    assert_eq!(rig.s.open_windows("demo"), 1);
    assert!(rig.s.tabs_of("/repo").iter().any(|t| t.id == id));
}

#[tokio::test]
async fn an_attached_empty_registry_round_trips_plugin_prefs() {
    let mut rig = rig();
    let root = rig.mkdir("plugins");
    let data = rig.mkdir("plugin-data");
    rig.s.prefs.plugins = json!({"disabled": ["x"]}).as_object().unwrap().clone();
    rig.s.attach_plugins(mx_plugins::Plugins::with_dirs(root.into(), data.into()));
    assert!(rig.s.rail_plugins().is_empty());
    rig.s.toggle_groups_collapsed();
    assert_eq!(rig.written()["plugins"], json!({"disabled": ["x"]}));
}
