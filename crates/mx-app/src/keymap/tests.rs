use std::collections::HashSet;

use mx_plugins::chord::{Chord, Key, KeyPress};
use serde_json::json;

use super::chord::{chord_id, press, slot_of};
use super::dispatch::{Focus, Hit, resolve};
use super::settings::{edit, shortcut_rows, stolen_banner};
use super::*;
use crate::chrome::settings::{ChordCheck, KeyPress as RecPress, SettingsAction};

fn chord(id: &str) -> Chord {
    Chord::parse(id).unwrap()
}

fn rec(key: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> RecPress {
    RecPress { key: key.into(), ctrl, alt, shift, meta }
}

fn all_defaults(linux: bool) -> Vec<Chord> {
    Action::ALL.iter().flat_map(|a| a.defaults(linux)).collect()
}

fn pressed(id: &str) -> KeyPress {
    let c = chord(id);
    press(c.key, c.control, c.alt, c.shift, c.meta)
}

fn no_plugin(_: &KeyPress) -> Option<String> {
    None
}

#[test]
fn cmd_t_labels_as_glyphs_and_round_trips_its_id() {
    let c = chord("meta+t");
    assert_eq!(c.label(false), "⌘T");
    assert_eq!(chord_id(&c), "meta+t");
    assert_eq!(Chord::parse(&chord_id(&c)), Some(c));
}

#[test]
fn every_default_of_every_action_round_trips() {
    for linux in [false, true] {
        for c in all_defaults(linux) {
            assert_eq!(Chord::parse(&chord_id(&c)), Some(c), "{}", chord_id(&c));
        }
    }
}

#[test]
fn alt_cmd_right_and_ctrl_tab_label_and_id() {
    let c = chord("alt+meta+right");
    assert_eq!(c.label(false), "⌥⌘→");
    assert_eq!(chord_id(&c), "alt+meta+right");
    assert_eq!(chord("ctrl+tab").label(false), "⌃⇥");
}

#[test]
fn bare_g_is_rejected_and_bare_f5_is_allowed() {
    assert!(chord("g").rejection(false).is_some());
    assert!(chord("f5").rejection(false).is_none());
}

#[test]
fn ctrl_c_is_rejected_and_ctrl_tab_is_allowed() {
    assert!(chord("ctrl+c").rejection(false).is_some());
    assert!(chord("ctrl+tab").rejection(false).is_none());
}

#[test]
fn cmd_q_rejection_names_the_chord() {
    assert!(chord("meta+q").rejection(false).unwrap().contains("⌘Q"));
}

#[test]
fn unknown_or_empty_ids_do_not_parse() {
    for id in ["hyper+t", "meta+teclamágica", ""] {
        assert_eq!(Chord::parse(id), None, "{id}");
    }
}

fn assert_sane_defaults(linux: bool) {
    let all = all_defaults(linux);
    let unique: HashSet<_> = all.iter().collect();
    assert_eq!(unique.len(), all.len());
    assert!(all.iter().all(|c| c.rejection(linux).is_none()));
    assert!(Action::ALL.iter().all(|a| !a.defaults(linux).is_empty()));
    let ids: HashSet<_> = Action::ALL.iter().map(|a| a.id()).collect();
    assert_eq!(ids.len(), Action::ALL.len());
}

#[test]
fn mac_defaults_have_one_owner_each_and_none_is_rejected() {
    assert_sane_defaults(false);
}

#[test]
fn linux_defaults_have_no_meta_no_duplicates_and_avoid_fixed_keys() {
    assert_sane_defaults(true);
    let fixed: Vec<Chord> = paste_chords(true)
        .into_iter()
        .chain([copy_chord(true)])
        .chain((0..9).map(|i| Chord::slot(i, true)))
        .collect();
    for c in all_defaults(true) {
        assert!(!c.meta);
        assert!(!fixed.contains(&c));
    }
}

#[test]
fn linux_labels_ids_slot_and_paste() {
    let c = Action::Claude.defaults(true)[0];
    assert_eq!(c.label(true), "Ctrl+Shift+T");
    assert_eq!(chord_id(&c), "ctrl+shift+t");
    assert_eq!(Chord::slot(0, true).label(true), "Alt+1");
    assert_eq!(chord("ctrl+pagedown").label(true), "Ctrl+PgDn");
    assert_eq!(paste_chords(true)[0].label(true), "Ctrl+Shift+V");
}

#[test]
fn linux_rejections() {
    assert_eq!(chord("ctrl+shift+j").rejection(true), None);
    assert!(chord("ctrl+j").rejection(true).unwrap().contains("Ctrl+J"));
    assert!(chord("ctrl+shift+e").rejection(true).unwrap().contains("IBus"));
    assert!(chord("ctrl+alt+left").rejection(true).is_some());
}

#[test]
fn linux_meta_plugin_chords_become_ctrl_shift() {
    assert_eq!(chord("meta+alt+g").for_host(true).label(true), "Ctrl+Shift+Alt+G");
    assert_eq!(chord("ctrl+alt+g").for_host(true), chord("ctrl+alt+g"));
}

#[test]
fn binding_a_taken_chord_steals_it() {
    let mut m = Keymap::defaults(false);
    assert_eq!(m.bind(Action::Shell, chord("meta+t"), None), Some(Action::Claude));
    assert!(!m.chords(Action::Claude).contains(&chord("meta+t")));
    assert_eq!(m.owner(&chord("meta+t")), Some(Action::Shell));
}

#[test]
fn replacing_keeps_the_position_and_rebinding_appends() {
    let mut m = Keymap::defaults(false);
    m.bind(Action::Claude, chord("meta+j"), Some(chord("meta+t")));
    assert_eq!(m.chords(Action::Claude), [chord("meta+j")]);
    m.bind(Action::Claude, chord("meta+t"), None);
    assert_eq!(m.chords(Action::Claude), [chord("meta+j"), chord("meta+t")]);
}

#[test]
fn reset_action_takes_its_defaults_back_from_the_thief() {
    let mut m = Keymap::defaults(false);
    m.bind(Action::Shell, chord("meta+t"), None);
    m.reset_action(Action::Claude);
    assert!(m.is_default(Action::Claude));
    assert!(!m.chords(Action::Shell).contains(&chord("meta+t")));
}

#[test]
fn fresh_keymap_saves_nothing_and_a_new_key_saves_the_whole_list() {
    let mut m = Keymap::defaults(false);
    assert!(m.to_json().is_empty());
    m.bind(Action::Refresh, chord("f5"), None);
    assert_eq!(serde_json::Value::Object(m.to_json()), json!({"refresh": ["meta+r", "f5"]}));
}

#[test]
fn saved_json_loads_back_into_the_same_lists() {
    let mut m = Keymap::defaults(false);
    m.bind(Action::Refresh, chord("f5"), None);
    m.bind(Action::Shell, chord("meta+t"), None);
    let back = Keymap::from_obj(&m.to_json(), false);
    assert_eq!(back, m);
    assert!(!back.all_default());
}

#[test]
fn a_chord_saved_twice_goes_to_the_first_action() {
    let m = Keymap::load(&json!({"claude": ["meta+g"], "shell": ["meta+g"]}), false);
    assert_eq!(m.chords(Action::Claude), [chord("meta+g")]);
    assert!(m.chords(Action::Shell).is_empty());
}

#[test]
fn invalid_saved_chords_are_dropped() {
    let m = Keymap::load(&json!({"claude": ["meta+q", "g", "meta+alt+g"]}), false);
    assert_eq!(m.chords(Action::Claude), [chord("meta+alt+g")]);
}

#[test]
fn non_map_saved_value_and_dictate_are_ignored() {
    assert!(Keymap::load(&json!(["meta+g"]), false).all_default());
    assert!(Keymap::load(&json!({"dictate": ["shift+meta+d"]}), false).all_default());
}

#[test]
fn reset_restores_every_default() {
    let mut m = Keymap::defaults(false);
    m.bind(Action::Refresh, chord("f5"), None);
    m.reset();
    assert!(m.all_default());
    assert!(m.to_json().is_empty());
}

#[test]
fn recorder_waits_on_modifiers_and_rejects_bare_letters() {
    assert_eq!(check_chord_for(&rec("shift", false, false, true, false), true), ChordCheck::Ignore);
    match check_chord_for(&rec("g", false, false, false, false), true) {
        ChordCheck::Rejected(why) => assert!(why.contains("sem modificador")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn recorder_accepts_shift_cmd_g_with_its_id_and_label() {
    assert_eq!(
        check_chord_for(&rec("g", false, false, true, true), false),
        ChordCheck::Accepted { id: "shift+meta+g".into(), label: "⇧⌘G".into() }
    );
}

#[test]
fn recorder_refuses_unknown_keys_and_slots() {
    assert!(matches!(check_chord_for(&rec("menu", true, false, false, false), true), ChordCheck::Rejected(_)));
    match check_chord_for(&rec("3", false, true, false, false), true) {
        ChordCheck::Rejected(why) => assert_eq!(why, "é a tecla da 3ª sessão da lateral"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn settings_bind_reports_the_stolen_owner() {
    let mut m = Keymap::defaults(true);
    let out = edit(&mut m, &SettingsAction::Bind { action: "shell".into(), chord: "ctrl+shift+t".into(), replacing: None });
    assert_eq!(out, Some(Some("Ctrl+Shift+T saiu de \"Nova sessão Claude\"".into())));
    assert_eq!(stolen_banner(&chord("meta+t"), Action::Claude, false), "⌘T saiu de \"Nova sessão Claude\"");
}

#[test]
fn settings_unbind_keeps_the_last_chord() {
    let mut m = Keymap::defaults(true);
    edit(&mut m, &SettingsAction::Unbind { action: "claude".into(), chord: "ctrl+shift+t".into() });
    assert_eq!(m.chords(Action::Claude).len(), 1);
    edit(&mut m, &SettingsAction::Unbind { action: "next-pane".into(), chord: "ctrl+tab".into() });
    assert_eq!(m.chords(Action::NextPane), [chord("ctrl+pagedown")]);
}

#[test]
fn shortcut_rows_follow_enum_order_and_flag_changes() {
    let mut m = Keymap::defaults(true);
    m.bind(Action::Refresh, chord("f5"), None);
    let rows = shortcut_rows(&m);
    assert_eq!(rows.len(), 20);
    assert_eq!(rows[0].action, "claude");
    let refresh = rows.iter().find(|r| r.action == "refresh").unwrap();
    assert!(refresh.changed);
    assert_eq!(refresh.chords[1], ("f5".into(), "F5".into()));
}

#[test]
fn a_keymap_chord_in_the_terminal_runs_its_action() {
    let m = Keymap::defaults(true);
    let f = Focus { terminal: true, text_field: false };
    assert_eq!(resolve(&m, &pressed("ctrl+shift+b"), f, no_plugin), Some(Hit::Action(Action::Sidebar)));
    assert_eq!(resolve(&m, &pressed("ctrl+shift+f"), f, no_plugin), Some(Hit::Action(Action::Search)));
    assert_eq!(resolve(&m, &pressed("ctrl+="), f, no_plugin), Some(Hit::Action(Action::ZoomIn)));
    assert_eq!(resolve(&m, &pressed("ctrl+0"), f, no_plugin), Some(Hit::Action(Action::ZoomReset)));
    assert_eq!(resolve(&m, &pressed("f2"), f, no_plugin), Some(Hit::Action(Action::Rename)));
}

#[test]
fn slot_chords_are_intercepted_inside_the_terminal() {
    let m = Keymap::defaults(true);
    let f = Focus { terminal: true, text_field: false };
    assert_eq!(resolve(&m, &pressed("alt+1"), f, no_plugin), Some(Hit::Slot(0)));
    assert_eq!(resolve(&m, &pressed("alt+9"), f, no_plugin), Some(Hit::Slot(8)));
    assert_eq!(slot_of(&pressed("alt+0"), true), None);
}

#[test]
fn fixed_terminal_keys_stay_in_the_terminal() {
    let mut m = Keymap::defaults(true);
    m.bind(Action::Refresh, chord("ctrl+shift+a"), None);
    let term = Focus { terminal: true, text_field: false };
    for id in ["shift+enter", "ctrl+shift+v", "ctrl+v", "shift+insert", "ctrl+shift+c", "ctrl+shift+a"] {
        assert_eq!(resolve(&m, &pressed_or_insert(id), term, no_plugin), None, "{id}");
    }
    assert_eq!(resolve(&m, &pressed("ctrl+shift+a"), Focus::default(), no_plugin), Some(Hit::Action(Action::Refresh)));
}

fn pressed_or_insert(id: &str) -> KeyPress {
    match Chord::parse(id) {
        Some(c) => press(c.key, c.control, c.alt, c.shift, c.meta),
        None => press(Key::Insert, false, false, true, false),
    }
}

#[test]
fn plain_typing_never_reaches_the_keymap() {
    let m = Keymap::defaults(true);
    let called = std::cell::Cell::new(false);
    let p = press(Key::Char('t'), false, false, true, false);
    assert_eq!(resolve(&m, &p, Focus { terminal: true, text_field: false }, |_| { called.set(true); None }), None);
    assert!(!called.get());
}

#[test]
fn editing_keys_in_a_text_field_skip_the_keymap() {
    let m = Keymap::defaults(false);
    let f = Focus { terminal: false, text_field: true };
    assert_eq!(resolve(&m, &pressed("meta+backspace"), f, no_plugin), None);
    assert_eq!(resolve(&m, &pressed("meta+backspace"), Focus::default(), no_plugin), Some(Hit::Action(Action::Close)));
}

#[test]
fn plugin_commands_only_run_when_the_keymap_misses() {
    let m = Keymap::defaults(true);
    let f = Focus { terminal: true, text_field: false };
    let plugin = |_: &KeyPress| Some("p/cmd".to_owned());
    assert_eq!(resolve(&m, &pressed("ctrl+shift+b"), f, plugin), Some(Hit::Action(Action::Sidebar)));
    assert_eq!(resolve(&m, &pressed("ctrl+shift+alt+g"), f, plugin), Some(Hit::Plugin("p/cmd".into())));
}

#[test]
fn terminal_fixed_keys_by_platform() {
    assert_eq!(terminal_key(&pressed("ctrl+shift+c"), true), Some(TerminalKey::Copy));
    assert_eq!(terminal_key(&pressed("meta+c"), false), Some(TerminalKey::Copy));
    assert_eq!(terminal_key(&pressed("meta+a"), false), Some(TerminalKey::SelectAll));
    assert_eq!(terminal_key(&pressed("ctrl+a"), true), None);
    assert_eq!(terminal_key(&pressed("ctrl+c"), true), None);
}
