use mx_core::model::{ClaudeStatus, FeatureOrHotfixKind, TabKind};
use serde_json::{Value, json};

use super::Rig;
use crate::HIBERNATED_AT_BIRTH;

fn load(rig: &mut Rig, config: Value) {
    let state = rig.s.opts.state_dir.clone();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("config.json"), config.to_string()).unwrap();
    rig.s.load();
}

#[test]
fn c1_state_lives_in_the_given_dir() {
    let rig = Rig::new();
    assert!(rig.s.options().config_path().starts_with(rig.dir.path()));
}

#[test]
fn c2_exited_shells_drop_but_resumable_sessions_stay() {
    let mut rig = Rig::new();
    rig.s1();
    rig.panel("c");
    rig.tm("c").session_id = Some("sess-do-fundo".into());
    rig.shell("s");
    for id in ["c", "s"] {
        rig.tm(id).exited = true;
    }
    rig.s.rename_tab("c", "c2");
    let j = rig.written();
    let ids: Vec<&Value> = j["layout"]["panes"].as_array().unwrap().iter().map(|p| &p["sessionId"]).collect();
    assert_eq!(ids, [&json!("sess-do-fundo")]);
}

#[test]
fn c3_one_atomic_write_leaves_no_temp_file() {
    let mut rig = Rig::new();
    rig.s1();
    rig.s.toggle_groups_collapsed();
    let j = rig.written();
    assert_eq!(j["folders"][0]["root"], "/repo");
    assert_eq!(j["groupsCollapsed"], true);
    let others: Vec<String> = std::fs::read_dir(&rig.s.opts.state_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("config.json") && n != "config.json")
        .collect();
    assert!(others.is_empty());
}

#[test]
fn c8_features_save_under_the_new_key() {
    let mut rig = Rig::new();
    rig.s1();
    rig.s.add_feature("/repo", "permissão do google", "", FeatureOrHotfixKind::Feature);
    let j = rig.written();
    assert!(j.get("projects").is_none());
    assert_eq!(j["featuresOrHotfixes"][0]["name"], "permissão do google");
}

#[test]
fn save_is_debounced_into_one_write() {
    let mut rig = Rig::new();
    rig.s1();
    let path = rig.s.opts.config_path();
    for w in [300.0, 310.0, 320.0] {
        rig.s.set_sidebar_width(w);
        rig.advance(chrono::TimeDelta::milliseconds(200));
    }
    assert!(!path.exists());
    rig.advance(chrono::TimeDelta::milliseconds(200));
    let j: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(j["sidebarWidth"], 320.0);
}

#[test]
fn dispose_discards_a_pending_write_and_flush_forces_it() {
    let mut rig = Rig::new();
    rig.s1();
    rig.s.set_sidebar_width(300.0);
    rig.s.flush();
    assert!(rig.s.opts.config_path().exists());
    std::fs::remove_file(rig.s.opts.config_path()).unwrap();
    rig.s.set_sidebar_width(310.0);
    rig.s.dispose();
    rig.advance(chrono::TimeDelta::seconds(1));
    assert!(!rig.s.opts.config_path().exists());
}

fn shells(rig: &Rig, n: usize) -> Value {
    let cwd = rig.dir.path().to_str().unwrap();
    let panes: Vec<Value> = (0..n)
        .map(|i| json!({"folderRoot": cwd, "kind": "shell", "cwd": cwd, "label": format!("p{i}")}))
        .collect();
    json!({"folders": [{"root": cwd, "name": "f", "collapsed": false}], "layout": {"panes": panes}})
}

fn labels(rig: &Rig) -> Vec<String> {
    rig.s.order().iter().map(|id| rig.t(id).custom_label.clone().unwrap()).collect()
}

#[tokio::test]
async fn restore_reads_the_legacy_two_slot_layout() {
    let mut rig = Rig::new();
    let mut c = shells(&rig, 3);
    c["layout"]["active"] = json!(2);
    c["layout"]["split"] = json!(0);
    load(&mut rig, c);
    assert_eq!(labels(&rig), ["p2", "p0"]);
}

#[tokio::test]
async fn restore_without_tree_or_active_leaves_the_screen_empty() {
    let mut rig = Rig::new();
    let c = shells(&rig, 2);
    load(&mut rig, c);
    assert_eq!(rig.s.tabs().len(), 2);
    assert!(rig.s.panes().is_none());
}

#[tokio::test]
async fn restore_shifts_leaves_past_a_missing_cwd_and_focuses_first_on_minus_one() {
    let mut rig = Rig::new();
    let mut c = shells(&rig, 3);
    c["layout"]["panes"][1]["cwd"] = json!("/nao/existe");
    c["layout"]["tree"] = json!({"axis": "row", "weights": [0.3, 0.3, 0.4], "children": [0, 1, 2]});
    c["layout"]["focused"] = json!(-1);
    load(&mut rig, c);
    assert_eq!(labels(&rig), ["p0", "p2"]);
    let first = rig.s.order()[0].clone();
    assert_eq!(rig.s.focused_pane_id(), Some(first.as_str()));
}

#[tokio::test]
async fn restore_drops_browsers_runs_unknown_kinds_as_shells_and_reads_project_root() {
    let mut rig = Rig::new();
    let cwd = rig.dir.path().to_str().unwrap().to_owned();
    let c = json!({
        "folders": [{"root": cwd, "name": "f", "collapsed": false}],
        "layout": {"panes": [
            {"projectRoot": cwd, "kind": "browser", "cwd": cwd},
            {"projectRoot": cwd, "kind": "esquisito", "cwd": cwd},
        ], "tree": 1, "focused": 0}
    });
    load(&mut rig, c);
    assert_eq!(rig.s.tabs().len(), 1);
    assert_eq!(rig.s.tabs()[0].kind, TabKind::Shell);
    assert_eq!(rig.s.tabs()[0].folder_root, cwd);
}

#[tokio::test]
async fn restore_parks_hibernated_sessions_and_runs_the_rest() {
    let mut rig = Rig::new();
    let cwd = rig.dir.path().to_str().unwrap().to_owned();
    let c = json!({
        "folders": [{"root": cwd, "name": "f", "collapsed": false}],
        "layout": {"panes": [
            {"folderRoot": cwd, "kind": "claude", "cwd": cwd, "sessionId": "s1", "hibernated": true},
            {"folderRoot": cwd, "kind": "claude", "cwd": cwd, "hibernated": true},
        ], "tree": {"axis": "row", "weights": [0.5, 0.5], "children": [0, 1]}, "focused": 0}
    });
    load(&mut rig, c);
    let (a, b) = (rig.s.tabs()[0].id.clone(), rig.s.tabs()[1].id.clone());
    assert!(rig.t(&a).hibernated && rig.t(&a).exited);
    assert_eq!(rig.t(&a).status(), ClaudeStatus::Ended);
    assert_eq!(rig.terms.log(&a).remarks, [HIBERNATED_AT_BIRTH]);
    assert!(rig.terms.log(&a).commands.is_empty());
    assert!(!rig.t(&b).hibernated);
    assert_eq!(rig.terms.log(&b).commands.len(), 1);
    assert!(!rig.s.save_pending());
}

#[tokio::test]
async fn restore_brings_back_history_marks_and_known_groups_only() {
    let mut rig = Rig::new();
    let cwd = rig.dir.path().to_str().unwrap().to_owned();
    let c = json!({
        "folders": [{"root": cwd, "name": "f", "collapsed": false}],
        "groups": [{"id": "g1", "name": "grid", "panes": [{"folderRoot": cwd, "kind": "shell", "cwd": cwd}]}],
        "layout": {"panes": [
            {"folderRoot": cwd, "kind": "shell", "cwd": cwd, "group": "g1", "touched": ["/a"], "done": true, "pinned": true,
             "plans": [{"text": "# p", "at": "2026-09-18T11:00:00.000"}],
             "suggestions": [{"id": "task_0000000a", "title": "t", "tldr": "", "prompt": "faz"}]},
            {"folderRoot": cwd, "kind": "shell", "cwd": cwd, "group": "sumiu", "zoom": 40, "tint": "cyan"},
        ], "tree": 0, "focused": 0}
    });
    load(&mut rig, c);
    let (a, b) = (rig.s.tabs()[0].clone(), rig.s.tabs()[1].clone());
    assert_eq!(a.group_id.as_deref(), Some("g1"));
    assert!(a.done && a.pinned);
    assert_eq!((a.hooks.touched.len(), a.hooks.plans.len(), a.hooks.suggestions.len()), (1, 1, 1));
    assert!(b.group_id.is_none());
    assert_eq!(b.zoom, 11);
    assert_eq!(b.tint, Some(mx_core::model::Tint::Cyan));
}

#[test]
fn unparseable_config_is_backed_up() {
    let mut rig = Rig::new();
    let state = rig.s.opts.state_dir.clone();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("config.json"), "{ quebrado").unwrap();
    assert!(!rig.s.load());
    assert!(state.join("config.json.bak").exists());
}

#[test]
fn retired_theme_ids_map_forward() {
    let mut rig = Rig::new();
    load(&mut rig, json!({"folders": [], "theme": "rose-pine"}));
    assert_eq!(rig.s.prefs().theme, "catppuccin-mocha");
    load(&mut rig, json!({"folders": [], "theme": "nada"}));
    assert_eq!(rig.s.prefs().theme, "maestria");
}
