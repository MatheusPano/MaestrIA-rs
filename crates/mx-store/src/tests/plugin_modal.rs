use std::sync::Arc;

use mx_plugins::rpc::{RfwUpdate, ViewOpen, ViewUpdate};
use mx_plugins::{PluginInfo, PluginManifest, PluginState};
use serde_json::json;

use super::Rig;
use crate::changes::Areas;

fn info() -> PluginInfo {
    let m = json!({"id": "wiboor", "name": "Wiboor", "version": "1", "main": ["node", "main.js"]});
    let manifest = PluginManifest::parse(m.as_object().unwrap()).unwrap();
    PluginInfo {
        id: "wiboor".into(),
        dir: "/p/wiboor".into(),
        data_dir: "/d/wiboor".into(),
        manifest: Some(Arc::new(manifest)),
        problem: None,
        linked: false,
        enabled: true,
        crash: None,
        state: PluginState::Idle,
        palettes: Vec::new(),
    }
}

fn rig() -> Rig {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("s");
    rig.s.look("s");
    rig
}

fn modal(from: Option<&str>, width: Option<f64>, height: Option<f64>) -> ViewOpen {
    let rfw = RfwUpdate { library: Some("widget root = Text(text: \"x\");".into()), root: None, data: None };
    ViewOpen {
        view_id: "chat".into(),
        title: "Chat".into(),
        rfw: Some(rfw),
        modal: true,
        from: from.map(str::to_owned),
        width,
        height,
        ..ViewOpen::default()
    }
}

#[test]
fn a_fresh_modal_keeps_from_width_and_height() {
    let mut rig = rig();
    rig.s.view_open(&info(), modal(Some("agente:1"), Some(500.), Some(400.)));
    let (_, v) = rig.s.modal().unwrap();
    assert_eq!(v.modal_from.as_deref(), Some("agente:1"));
    assert_eq!((v.modal_width, v.modal_height), (Some(500.), Some(400.)));
}

#[test]
fn reopening_the_open_modal_only_updates_from_and_keeps_it_without_one() {
    let mut rig = rig();
    rig.s.view_open(&info(), modal(Some("agente:1"), Some(500.), Some(400.)));
    rig.s.view_open(&info(), modal(Some("agente:2"), Some(900.), None));
    let (_, v) = rig.s.modal().unwrap();
    assert_eq!(v.modal_from.as_deref(), Some("agente:2"));
    assert_eq!((v.modal_width, v.modal_height), (Some(500.), Some(400.)));
    rig.s.view_open(&info(), modal(None, None, None));
    assert_eq!(rig.s.modal().unwrap().1.modal_from.as_deref(), Some("agente:2"));
    rig.s.close_modal();
    rig.s.view_open(&info(), modal(None, None, None));
    let (_, v) = rig.s.modal().unwrap();
    assert_eq!((v.modal_from.as_deref(), v.modal_width), (None, None));
}

fn data_update(title: Option<&str>) -> ViewUpdate {
    let data = json!({"label": "dois"}).as_object().cloned();
    ViewUpdate {
        view_id: "chat".into(),
        title: title.map(str::to_owned),
        blocks: None,
        rfw: Some(RfwUpdate { library: None, root: None, data }),
    }
}

#[test]
fn perf1_a_data_only_update_reaches_only_the_plugin_windows() {
    let mut rig = rig();
    let id = rig.s.view_open(&info(), ViewOpen { modal: false, ..modal(None, None, None) });
    rig.s.take_changes();
    assert!(rig.s.view_update("wiboor", data_update(None)));
    let c = rig.s.take_changes();
    assert!(c.areas.contains(Areas::PLUGINS));
    assert!(!c.areas.intersects(Areas::PANES | Areas::SIDEBAR) && c.tabs.is_empty(), "{c:?}");
    assert_eq!(rig.s.plugin_view(&id).unwrap().rfw_data.get("label"), Some(&json!("dois")));
    assert!(rig.s.view_update("wiboor", data_update(Some("Chat"))));
    assert!(!rig.s.take_changes().areas.intersects(Areas::PANES));
    assert!(rig.s.view_update("wiboor", data_update(Some("outra"))));
    let c = rig.s.take_changes();
    assert!(c.areas.contains(Areas::PANES) && c.tabs == [id.clone()]);
    assert_eq!(rig.s.title(rig.t(&id)), "outra");
}
