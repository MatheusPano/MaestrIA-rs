use std::sync::Arc;

use mx_core::model::Folder;
use mx_store::testing::{FakeBus, FakeClock, FakeHooks, FakeNotifier, FakeTerms};
use mx_store::{Ports, Store, StoreOptions};
use mx_sys::FakeRunner;

use super::dispatch::{NO_FOLDER_FOR_TASK, NOT_CLAUDE, UiRequest, run_action};
use super::*;
use super::chord::chord_id;
use super::dispatch::{Hit, run};
use crate::chrome::settings::SettingsAction;

struct Rig {
    s: Store,
    _rt: tokio::runtime::Runtime,
    dir: std::path::PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn rig(name: &str) -> Rig {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let dir = std::env::temp_dir().join(format!("mx-keymap-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ports = Ports {
        clock: Arc::new(FakeClock::new(chrono::Local::now())),
        terms: Arc::new(FakeTerms::default()),
        runner: Arc::new(FakeRunner::new()),
        notifier: Arc::new(FakeNotifier::default()),
        plugins: Arc::new(FakeBus::default()),
        hooks: Arc::new(FakeHooks { port: 4242 }),
        rt: rt.handle().clone(),
    };
    let opts = StoreOptions::at(dir.join("state"), dir.join("chats"), dir.to_str());
    Rig { s: Store::new(opts, ports), _rt: rt, dir }
}

fn shell(s: &mut Store, label: &str) -> String {
    let id = s.open_shell(&Folder::new("/repo", "repo"), None, None, None, None);
    s.rename_tab(&id, label);
    id
}

#[test]
fn f2_on_a_focused_panel_asks_for_the_rename_dialog() {
    let mut r = rig("rename");
    let id = shell(&mut r.s, "permissão do google");
    let ui = run_action(&mut r.s, Action::Rename);
    assert_eq!(ui, Some(UiRequest::Rename { tab_id: id, title: "permissão do google".into() }));
}

#[test]
fn rename_without_a_focused_panel_explains_in_a_banner() {
    let mut r = rig("rename-none");
    assert_eq!(run_action(&mut r.s, Action::Rename), None);
    assert!(r.s.banner().unwrap().contains("renomear"));
}

#[test]
fn plan_on_a_shell_says_it_is_not_claude() {
    let mut r = rig("plan");
    shell(&mut r.s, "x");
    run_action(&mut r.s, Action::Plan);
    assert_eq!(r.s.banner(), Some(NOT_CLAUDE));
}

#[test]
fn new_task_needs_a_real_folder() {
    let mut r = rig("task");
    assert_eq!(run_action(&mut r.s, Action::Task), None);
    assert_eq!(r.s.banner(), Some(NO_FOLDER_FOR_TASK));
}

#[test]
fn sidebar_toggles_and_search_brings_it_back() {
    let mut r = rig("sidebar");
    run_action(&mut r.s, Action::Sidebar);
    assert!(r.s.prefs().sidebar_hidden);
    assert_eq!(run_action(&mut r.s, Action::Search), Some(UiRequest::FocusSearch));
    assert!(!r.s.prefs().sidebar_hidden);
    assert_eq!(run_action(&mut r.s, Action::Search), Some(UiRequest::FocusSearch));
    assert!(!r.s.prefs().sidebar_hidden);
}

#[test]
fn zoom_keys_change_only_the_focused_panel() {
    let mut r = rig("zoom");
    let a = shell(&mut r.s, "a");
    let b = shell(&mut r.s, "b");
    r.s.focus_pane(&b);
    run_action(&mut r.s, Action::ZoomIn);
    run_action(&mut r.s, Action::ZoomIn);
    assert_eq!(r.s.tab(&b).unwrap().zoom, 2);
    assert_eq!(r.s.tab(&a).unwrap().zoom, 0);
    run_action(&mut r.s, Action::ZoomReset);
    assert_eq!(r.s.tab(&b).unwrap().zoom, 0);
}

#[test]
fn slot_hit_selects_the_nth_session() {
    let mut r = rig("slot");
    let a = shell(&mut r.s, "a");
    shell(&mut r.s, "b");
    run(&mut r.s, &Hit::Slot(0));
    assert_eq!(r.s.focused_tab_id(), Some(a));
}

#[test]
fn settings_edits_persist_in_shortcuts_and_announce_the_theft() {
    let mut r = rig("settings");
    let bind = SettingsAction::Bind { action: "shell".into(), chord: chord_id(&Action::Claude.defaults(LINUX)[0]), replacing: None };
    assert!(apply(&mut r.s, &bind));
    assert!(r.s.prefs().shortcuts.contains_key("claude"));
    assert!(r.s.banner().unwrap().contains("saiu de \"Nova sessão Claude\""));
    assert!(apply(&mut r.s, &SettingsAction::ResetShortcuts(None)));
    assert!(r.s.prefs().shortcuts.is_empty());
    assert!(!apply(&mut r.s, &SettingsAction::NewLauncher));
}
