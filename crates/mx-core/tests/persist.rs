use std::fs;

use chrono::{Local, TimeZone};
use mx_core::json::Obj;
use mx_core::layout::{Axis, PaneNode};
use mx_core::model::sidebar::{RowKey, folders_of, sidebar_rows};
use mx_core::model::{FeatureOrHotfix, FeatureOrHotfixKind, Folder, Workspace};
use mx_core::persist::*;
use mx_core::text::Time;
use serde_json::{Value, json};

fn now() -> Time {
    Local.with_ymd_and_hms(2026, 9, 18, 12, 0, 0).unwrap()
}

fn obj(v: Value) -> Obj {
    v.as_object().unwrap().clone()
}

fn fixture() -> Obj {
    let text = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/config-2.4.0.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn ws_named<'a>(s: &'a Sidebar, name: &str) -> &'a Workspace {
    s.workspaces.iter().find(|w| w.name == name).unwrap()
}

#[test]
fn fixture_2_4_0_keeps_the_atrium_workspace_with_its_stamped_folders() {
    let s = read_sidebar(&fixture());
    let atrium = ws_named(&s, "atrium");
    assert_eq!(
        atrium.code_workspace_path.as_deref(),
        Some("/repos/atrium.code-workspace")
    );
    assert_eq!(
        atrium.folder_roots,
        ["/repos/atrium-backend", "/repos/atrium-frontend"]
    );
    assert!(atrium.collapsed);
}

#[test]
fn fixture_2_4_0_creates_the_uplii_workspace_from_the_stamp() {
    let s = read_sidebar(&fixture());
    let uplii = ws_named(&s, "uplii");
    assert_eq!(
        uplii.code_workspace_path.as_deref(),
        Some("/repos/uplii.code-workspace")
    );
    assert_eq!(uplii.folder_roots, ["/repos/uplii-ai"]);
}

#[test]
fn fixture_2_4_0_orders_rows_where_the_first_folder_was() {
    let s = read_sidebar(&fixture());
    let rows = sidebar_rows(&s.folders, &s.workspaces, &s.root_order);
    let atrium = ws_named(&s, "atrium").id.clone();
    let uplii = ws_named(&s, "uplii").id.clone();
    assert_eq!(
        rows,
        [
            RowKey::Workspace(atrium),
            RowKey::Folder("/repos/solta".into()),
            RowKey::Workspace(uplii)
        ]
    );
    assert_eq!(s.features_or_hotfixes.len(), 1);
    assert_eq!(
        s.features_or_hotfixes[0].folder_root,
        "/repos/atrium-backend"
    );
}

#[test]
fn fixture_2_4_0_moves_folder_collapse_into_the_workspace() {
    let s = read_sidebar(&fixture());
    assert!(ws_named(&s, "atrium").is_folder_collapsed("/repos/atrium-frontend"));
    assert!(!ws_named(&s, "atrium").is_folder_collapsed("/repos/atrium-backend"));
}

#[test]
fn sidebar_rows_put_a_workspace_where_its_first_folder_is() {
    let folders = vec![
        Folder::new("/r/api", "api"),
        Folder::new("/r/solta", "solta"),
        Folder::new("/r/web", "web"),
    ];
    let mut w = Workspace::new("ws1", "cefis");
    w.folder_roots = vec!["/r/api".into(), "/r/web".into()];
    let ws = vec![w];
    assert_eq!(
        sidebar_rows(&folders, &ws, &[]),
        [
            RowKey::Workspace("ws1".into()),
            RowKey::Folder("/r/solta".into())
        ]
    );
    let names: Vec<_> = folders_of(&ws[0], &folders)
        .iter()
        .map(|f| f.name.clone())
        .collect();
    assert_eq!(names, ["api", "web"]);
}

#[test]
fn sidebar_rows_skip_dangling_keys_and_append_unlisted_rows() {
    let folders = vec![
        Folder::new("/a", "a"),
        Folder::new("/b", "b"),
        Folder::new("/c", "c"),
    ];
    let mut w = Workspace::new("ws1", "W");
    w.folder_roots = vec!["/b".into(), "/sumiu".into()];
    let empty = Workspace::new("ws2", "vazio");
    let order: Vec<String> = [
        "folder:/c",
        "workspace:ghost",
        "folder:/b",
        "lixo",
        "folder:/nao",
        "workspace:ws2",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let keys: Vec<String> = sidebar_rows(&folders, &[w, empty], &order)
        .iter()
        .map(RowKey::key)
        .collect();
    assert_eq!(
        keys,
        ["folder:/c", "workspace:ws2", "folder:/a", "workspace:ws1"]
    );
}

#[test]
fn legacy_projects_become_features() {
    let s = read_sidebar(&obj(json!({
        "folders": [{"root": "/repo", "name": "repo"}],
        "projects": [{"id": "pj1", "folderRoot": "/repo", "name": "permissão do google"}]
    })));
    assert_eq!(s.features_or_hotfixes[0].id, "pj1");
    assert_eq!(s.features_or_hotfixes[0].name, "permissão do google");
    assert_eq!(s.features_or_hotfixes[0].kind, FeatureOrHotfixKind::Feature);
}

#[test]
fn features_or_hotfixes_win_over_legacy_projects() {
    let s = read_sidebar(&obj(json!({
        "folders": [],
        "projects": [{"id": "velho", "folderRoot": "/repo", "name": "v"}],
        "featuresOrHotfixes": [{"id": "novo", "folderRoot": "/repo", "name": "n"}]
    })));
    let ids: Vec<_> = s
        .features_or_hotfixes
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(ids, ["novo"]);
}

#[test]
fn pre_projects_format_reads_projects_as_folders() {
    let s = read_sidebar(&obj(
        json!({"projects": [{"root": "/repo", "name": "repo"}]}),
    ));
    assert_eq!(s.folders[0].root, "/repo");
    assert!(s.features_or_hotfixes.is_empty());
}

#[test]
fn feature_id_honours_the_legacy_project_key() {
    assert_eq!(
        feature_or_hotfix_id_in(&obj(json!({"featureOrHotfixId": "a"}))).as_deref(),
        Some("a")
    );
    assert_eq!(
        feature_or_hotfix_id_in(&obj(json!({"projectId": "b"}))).as_deref(),
        Some("b")
    );
    assert_eq!(
        feature_or_hotfix_id_in(&obj(json!({"featureOrHotfixId": "a", "projectId": "b"})))
            .as_deref(),
        Some("a")
    );
    assert_eq!(feature_or_hotfix_id_in(&obj(json!({}))), None);
}

#[test]
fn wrongly_typed_entries_are_skipped_not_fatal() {
    let (c, skipped) = Config::from_json(&obj(json!({
        "folders": [{"root": "/a", "name": "a"}, {"root": "/b", "name": 5}, "x"],
        "workspaces": [{"id": "w", "name": 5}, {"id": "ok", "name": "OK"}],
        "groups": [{"id": "g", "name": 7, "panes": [{}]}, {"id": "g2", "name": "G", "panes": [{"kind": "shell"}]}],
        "launchers": [{"id": "l", "name": ["x"], "command": "c"}],
        "sidebarHidden": "sim",
        "layout": {"panes": [{"folderRoot": "/a", "kind": "shell", "cwd": "/a"}], "tree": 0, "focused": 0}
    })));
    assert_eq!(c.folders.len(), 1);
    assert_eq!(c.workspaces.len(), 1);
    assert_eq!(c.groups.len(), 1);
    assert!(c.launchers.is_empty());
    assert!(!c.sidebar_hidden);
    assert_eq!(c.layout.unwrap().panes.len(), 1);
    assert!(skipped.0.len() >= 5);
}

#[test]
fn defaults_clamps_and_retired_themes() {
    let (c, _) = Config::from_json(&obj(
        json!({"sidebarWidth": 2000, "hibernateMinutes": 12.9, "type": {"size": 30, "line": 1.234}}),
    ));
    assert_eq!(c.sidebar_width, 680.0);
    assert_eq!(c.hibernate_minutes, 12);
    assert_eq!(c.typography.size, 24.0);
    assert_eq!(c.typography.line, 1.23);
    let (c, _) = Config::from_json(&obj(json!({"sidebarWidth": -40})));
    assert_eq!(c.sidebar_width, 248.0);
    let (c, _) = Config::from_json(&obj(json!({})));
    assert_eq!(c.sidebar_width, 352.0);
    assert_eq!(c.hibernate_minutes, 30);
    let known = |id: &str| ["maestria", "graphite", "catppuccin-mocha"].contains(&id);
    assert_eq!(resolve_theme(Some("rose-pine"), &known), "catppuccin-mocha");
    assert_eq!(resolve_theme(Some("chatgpt-dark"), &known), "graphite");
    assert_eq!(resolve_theme(Some("sumiu"), &known), "maestria");
    assert_eq!(resolve_theme(Some("graphite"), &known), "graphite");
    assert_eq!(resolve_theme(None, &known), "maestria");
}

#[test]
fn typography_writes_only_what_differs_and_clamps_zoom() {
    let t = Typography::default();
    assert!(t.to_json().is_empty());
    assert_eq!(t.clamp_zoom(-10), -4);
    assert_eq!(t.clamp_zoom(20), 11);
    let big = Typography::new(Some("Iosevka"), Some(14.0), None);
    assert_eq!(
        Value::Object(big.to_json()),
        json!({"mono": "Iosevka", "size": 14.0})
    );
}

fn current_config() -> Config {
    let mut folder = Folder::new("/home/u/repos/api", "api");
    folder.tint = Some(mx_core::model::Tint::Cyan);
    let mut ws = Workspace::new("wsm3abcd0", "ATRIUM");
    ws.folder_roots = vec!["/home/u/repos/api".into()];
    let feature = FeatureOrHotfix::new(
        "pjm3abce",
        "/home/u/repos/api",
        "login quebrado",
        FeatureOrHotfixKind::Hotfix,
    );
    let group = mx_core::model::PaneGroup::from_json(&json!({"id": "gpm3abcf", "name": "grid da manhã", "panes": [{"folderRoot": "/home/u/repos/api", "kind": "shell", "cwd": "/home/u/repos/api"}]})).unwrap();
    let launcher = mx_core::model::Launcher::from_json(
        &json!({"id": "lch1729500000000000", "name": "btop", "command": "btop", "icon": "monitor"}),
    )
    .unwrap();
    Config {
        folders: vec![folder],
        workspaces: vec![ws],
        root_order: vec!["workspace:wsm3abcd0".into()],
        features_or_hotfixes: vec![feature],
        groups: vec![group],
        launchers: vec![launcher],
        theme: Some("maestria".into()),
        layout: Some(Layout {
            panes: vec![obj(
                json!({"folderRoot": "/home/u/repos/api", "kind": "claude", "cwd": "/home/u/repos/api", "label": "TASK#1", "sessionId": "0b7c", "touched": ["/home/u/repos/api/a.dart"], "done": true}),
            )],
            tree: Some(json!(0)),
            focused: 0,
            legacy: None,
        }),
        ..Config::default()
    }
}

#[test]
fn writing_follows_the_spec_example_byte_for_byte() {
    let expected = r#"{"folders":[{"root":"/home/u/repos/api","name":"api","collapsed":false,"tint":"cyan"}],"workspaces":[{"id":"wsm3abcd0","name":"ATRIUM","folderRoots":["/home/u/repos/api"]}],"rootOrder":["workspace:wsm3abcd0"],"featuresOrHotfixes":[{"id":"pjm3abce","folderRoot":"/home/u/repos/api","name":"login quebrado","brief":"","collapsed":false,"kind":"hotfix"}],"groups":[{"id":"gpm3abcf","name":"grid da manhã","panes":[{"folderRoot":"/home/u/repos/api","kind":"shell","cwd":"/home/u/repos/api"}]}],"launchers":[{"id":"lch1729500000000000","name":"btop","command":"btop","icon":"monitor"}],"sidebarWidth":352.0,"theme":"maestria","layout":{"panes":[{"folderRoot":"/home/u/repos/api","kind":"claude","cwd":"/home/u/repos/api","label":"TASK#1","sessionId":"0b7c","touched":["/home/u/repos/api/a.dart"],"done":true}],"tree":0,"focused":0}}"#;
    assert_eq!(current_config().to_json_string(), expected);
}

#[test]
fn false_booleans_and_defaults_are_never_written() {
    let text = Config::default().to_json_string();
    assert_eq!(
        text,
        r#"{"folders":[],"rootOrder":[],"featuresOrHotfixes":[],"sidebarWidth":352.0,"theme":"maestria","layout":{"panes":[],"focused":-1}}"#
    );
    let on = Config {
        sidebar_hidden: true,
        status_bar_top: true,
        do_not_disturb: true,
        suggestions_to_plugin: true,
        groups_collapsed: true,
        hibernate_minutes: 0,
        rail_order: vec!["p".into()],
        sidebar_view: Some("p".into()),
        ..Config::default()
    };
    let keys: Vec<String> = on.to_json().as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        [
            "folders",
            "rootOrder",
            "featuresOrHotfixes",
            "sidebarWidth",
            "sidebarHidden",
            "statusBarTop",
            "railOrder",
            "sidebarView",
            "doNotDisturb",
            "suggestionsToPlugin",
            "groupsCollapsed",
            "hibernateMinutes",
            "theme",
            "layout"
        ]
    );
}

#[test]
fn the_written_root_order_is_the_derived_one() {
    let mut c = current_config();
    c.root_order = vec!["workspace:fantasma".into()];
    c.folders.push(Folder::new("/solta", "solta"));
    assert_eq!(
        c.to_json()["rootOrder"],
        json!(["workspace:wsm3abcd0", "folder:/solta"])
    );
}

#[test]
fn the_legacy_workspace_stamp_is_never_written() {
    let (c, _) = Config::from_json(&fixture());
    let text = c.to_json_string();
    assert!(!text.contains("\"workspace\":"));
    assert!(!text.contains("\"projects\""));
    assert!(text.contains("\"featuresOrHotfixes\":[{\"id\":\"pj1\""));
}

#[test]
fn the_dictation_key_is_dropped() {
    let (c, _) = Config::from_json(&obj(json!({"dictation": {"on": true}})));
    assert!(c.to_json().get("dictation").is_none());
}

#[test]
fn atomic_write_uses_a_pid_temp_file_and_leaves_only_config_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state/config.json");
    assert_eq!(
        temp_path(&path, 4242).file_name().unwrap(),
        "config.json.4242.tmp"
    );
    write_atomic(&path, "{}", 4242).unwrap();
    save(&path, &current_config()).unwrap();
    let names: Vec<String> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["config.json"]);
    let LoadOutcome::Loaded {
        config,
        skipped,
        backed_up,
    } = load(&path)
    else {
        panic!()
    };
    assert!(skipped.is_empty() && !backed_up);
    assert_eq!(config.to_json(), current_config().to_json());
}

#[test]
fn an_unparseable_config_is_backed_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    fs::write(&path, "{\"folders\": [").unwrap();
    let LoadOutcome::Unreadable { backed_up, .. } = load(&path) else {
        panic!()
    };
    assert!(backed_up);
    assert_eq!(
        fs::read_to_string(backup_path(&path)).unwrap(),
        "{\"folders\": ["
    );
    fs::write(&path, "[1]").unwrap();
    assert!(matches!(load(&path), LoadOutcome::Unreadable { .. }));
    assert!(matches!(
        load(&dir.path().join("nada.json")),
        LoadOutcome::Missing
    ));
}

#[test]
fn a_config_with_skipped_entries_is_also_backed_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    fs::write(
        &path,
        r#"{"folders":[{"root":"/a","name":"a"},{"root":"/b","name":5}]}"#,
    )
    .unwrap();
    let LoadOutcome::Loaded {
        config, backed_up, ..
    } = load(&path)
    else {
        panic!()
    };
    assert!(backed_up);
    assert_eq!(config.folders.len(), 1);
}

fn layout(j: Value) -> Layout {
    let (c, _) = Config::from_json(&obj(json!({"layout": j})));
    c.layout.unwrap()
}

fn ids(v: &[&str]) -> Vec<Option<String>> {
    v.iter()
        .map(|s| (!s.is_empty()).then(|| s.to_string()))
        .collect()
}

#[test]
fn restore_uses_the_saved_tree_and_focus() {
    let l = layout(
        json!({"panes": [{}, {}], "tree": {"axis": "column", "weights": [1, 3], "children": [0, 1]}, "focused": 1}),
    );
    let r = restore_tree(&l, &ids(&["a", "b"])).unwrap();
    let TreeRestore::Replace(Some(tree)) = r.tree else {
        panic!()
    };
    assert_eq!(tree.as_split().unwrap().axis, Axis::Column);
    assert_eq!(tree.as_split().unwrap().weights, [0.25, 0.75]);
    assert_eq!(r.focused.as_deref(), Some("b"));
}

#[test]
fn restore_shifts_past_panes_that_did_not_come_back() {
    let l = layout(
        json!({"panes": [{}, {}, {}], "tree": {"axis": "row", "weights": [0.2, 0.3, 0.5], "children": [0, 1, 2]}, "focused": -1}),
    );
    let r = restore_tree(&l, &ids(&["a", "", "c"])).unwrap();
    let TreeRestore::Replace(Some(tree)) = r.tree else {
        panic!()
    };
    assert_eq!(tree.order(), ["a", "c"]);
    assert_eq!(r.focused.as_deref(), Some("a"));
}

#[test]
fn restore_reads_the_legacy_two_slot_layout() {
    let l = layout(json!({"panes": [{}, {}], "active": 0, "split": 1, "focused": 1}));
    let r = restore_tree(&l, &ids(&["a", "b"])).unwrap();
    let expected = PaneNode::split(
        Axis::Row,
        vec![PaneNode::leaf("a"), PaneNode::leaf("b")],
        vec![0.5, 0.5],
    );
    assert_eq!(r.tree, TreeRestore::Replace(Some(expected)));
    let single = layout(json!({"panes": [{}], "active": null}));
    assert_eq!(
        restore_tree(&single, &ids(&["a"])).unwrap().tree,
        TreeRestore::Replace(Some(PaneNode::leaf("a")))
    );
    let gone = layout(json!({"panes": [{}, {}], "active": 0}));
    assert_eq!(
        restore_tree(&gone, &ids(&["", "b"])).unwrap().tree,
        TreeRestore::KeepPlaced
    );
}

#[test]
fn restore_without_tree_or_active_leaves_the_screen_empty() {
    let l = layout(json!({"panes": [{}], "focused": 0}));
    let r = restore_tree(&l, &ids(&["a"])).unwrap();
    assert_eq!(r.tree, TreeRestore::Replace(None));
    assert_eq!(r.focused.as_deref(), Some("a"));
    assert_eq!(restore_tree(&l, &ids(&[""])), None);
}

#[test]
fn pane_entries_read_legacy_keys_and_kinds() {
    let e = PaneEntry::parse(
        &obj(
            json!({"projectRoot": "/repo", "projectId": "pj1", "kind": "browser", "cwd": "/repo"}),
        ),
        now(),
    );
    assert_eq!(e.folder_root.as_deref(), Some("/repo"));
    assert_eq!(e.feature_or_hotfix_id.as_deref(), Some("pj1"));
    assert_eq!(e.kind, PaneKind::Browser);
    assert_eq!(PaneKind::parse(Some("plugin")), PaneKind::Shell);
    assert_eq!(PaneKind::parse(None), PaneKind::Shell);
    let e = PaneEntry::parse(
        &obj(
            json!({"folderRoot": "/n", "projectRoot": "/o", "kind": "claude", "sessionId": "s", "hibernated": true,
            "zoom": 3.7, "tint": "blue", "touched": ["/a", 3], "plans": [{"text": "p"}, {"text": ""}],
            "suggestions": [{"id": "task_1", "prompt": "x"}], "done": true, "pinned": "yes", "group": "gp1"}),
        ),
        now(),
    );
    assert_eq!(e.folder_root.as_deref(), Some("/n"));
    assert!(e.asleep());
    assert_eq!(e.zoom, 3);
    assert_eq!(e.tint, None);
    assert_eq!(e.touched, ["/a"]);
    assert_eq!(e.plans.len(), 1);
    assert_eq!(e.suggestions.len(), 1);
    assert!(e.done && !e.pinned);
    assert_eq!(e.group.as_deref(), Some("gp1"));
    let reader = PaneEntry::parse(&obj(json!({"kind": "reader", "cwd": "/r"})), now());
    assert_eq!(reader.doc, None);
}
