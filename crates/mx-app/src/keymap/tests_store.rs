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
fn a_panel_hit_selects_that_panel() {
    let mut r = rig("panel");
    let a = shell(&mut r.s, "a");
    shell(&mut r.s, "b");
    run(&mut r.s, &Hit::Panel(a.clone()));
    assert_eq!(r.s.focused_tab_id(), Some(a));
}

#[test]
fn a_new_tab_has_no_hotkey_and_no_digit_reaches_a_panel() {
    let mut r = rig("fresh");
    let a = shell(&mut r.s, "a");
    assert_eq!(r.s.tab(&a).unwrap().hotkey, None);
    let p = super::chord::press(mx_plugins::chord::Key::Char('1'), false, true, false, true);
    assert_eq!(super::dispatch::panel_hit(&r.s, &p), None);
}

#[test]
fn giving_a_panel_the_key_of_another_steals_it_and_names_the_old_owner() {
    let mut r = rig("steal");
    let a = shell(&mut r.s, "primeiro");
    let b = shell(&mut r.s, "segundo");
    r.s.set_hotkey(&a, Some("alt+meta+g"));
    r.s.set_hotkey(&b, Some("alt+meta+g"));
    assert_eq!(r.s.tab(&a).unwrap().hotkey, None);
    assert_eq!(r.s.tab(&b).unwrap().hotkey.as_deref(), Some("alt+meta+g"));
    assert!(r.s.banner().unwrap().contains("primeiro"));
}

#[test]
fn removing_a_panel_hotkey_clears_it() {
    let mut r = rig("remove");
    let a = shell(&mut r.s, "primeiro");
    r.s.set_hotkey(&a, Some("alt+meta+g"));
    r.s.set_hotkey(&a, None);
    assert_eq!(r.s.tab(&a).unwrap().hotkey, None);
}

#[test]
fn binding_an_action_to_a_panel_key_takes_it_from_the_panel() {
    let mut r = rig("bind-panel");
    let a = shell(&mut r.s, "primeiro");
    r.s.set_hotkey(&a, Some("alt+meta+g"));
    let bind = SettingsAction::Bind { action: "markdown".into(), chord: "alt+meta+g".into(), replacing: None };
    assert!(apply(&mut r.s, &bind));
    assert_eq!(r.s.tab(&a).unwrap().hotkey, None);
    assert!(r.s.banner().unwrap().contains("primeiro"));
}

#[test]
fn the_hotkey_lives_in_the_layout_and_not_in_the_recipe() {
    let mut r = rig("json");
    let a = shell(&mut r.s, "primeiro");
    r.s.set_hotkey(&a, Some("alt+meta+g"));
    let t = r.s.tab(&a).unwrap();
    assert_eq!(t.to_json()["hotkey"], "alt+meta+g");
    let back = mx_plugins::chord::Chord::parse("alt+meta+g").unwrap();
    assert_eq!(chord_id(&back), "alt+meta+g");
    assert!(!t.recipe().contains_key("hotkey"));
}

#[test]
fn a_panel_with_a_key_shows_up_in_the_panel_bindings() {
    let mut r = rig("bindings");
    shell(&mut r.s, "a");
    let b = shell(&mut r.s, "b");
    r.s.set_hotkey(&b, Some("alt+meta+g"));
    let p = super::chord::press(mx_plugins::chord::Key::Char('g'), false, true, false, true);
    assert_eq!(super::dispatch::panel_hit(&r.s, &p), Some(b));
}

#[test]
fn find_in_pane_asks_the_ui_for_the_focused_terminal() {
    let mut r = rig("find");
    assert_eq!(run_action(&mut r.s, Action::FindInPane), Some(UiRequest::FindInPane { tab_id: None }));
    let a = shell(&mut r.s, "a");
    assert_eq!(run_action(&mut r.s, Action::FindInPane), Some(UiRequest::FindInPane { tab_id: Some(a) }));
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

#[test]
fn a_saved_hotkey_that_an_action_took_meanwhile_stays_with_the_action() {
    let mut r = rig("taken");
    let a = shell(&mut r.s, "a");
    let b = shell(&mut r.s, "b");
    r.s.set_hotkey(&a, Some("ctrl+shift+b"));
    r.s.set_hotkey(&b, Some("ctrl+alt+g"));
    super::hotkey::drop_taken_hotkeys(&mut r.s);
    assert_eq!(r.s.tab(&a).unwrap().hotkey, None);
    assert_eq!(r.s.tab(&b).unwrap().hotkey.as_deref(), Some("ctrl+alt+g"));
}

#[test]
fn the_panel_recorder_refuses_action_and_plugin_keys_and_takes_free_ones() {
    use super::hotkey::{HotkeyCheck, judge};
    use crate::chrome::settings::KeyPress;
    let m = Keymap::defaults(true);
    let press = |key: &str, ctrl, alt, shift| KeyPress { key: key.into(), ctrl, alt, shift, meta: false };
    let none = |_: &mx_plugins::chord::Chord| None;
    assert_eq!(judge(&press("shift", false, false, true), &m, none), HotkeyCheck::Ignore);
    assert_eq!(
        judge(&press("b", true, false, true), &m, none),
        HotkeyCheck::Rejected("Ctrl+Shift+B: já é o atalho de \"Esconder ou mostrar a lateral\"".into())
    );
    assert_eq!(
        judge(&press("g", true, true, false), &m, |_| Some("Status".into())),
        HotkeyCheck::Rejected("Ctrl+Alt+G: já é a tecla de \"Status\", de um plugin".into())
    );
    assert!(matches!(judge(&press("g", false, false, false), &m, none), HotkeyCheck::Rejected(why) if why.starts_with("G: ")));
    assert_eq!(judge(&press("1", false, true, false), &m, none), HotkeyCheck::Accepted("alt+1".into()));
}
