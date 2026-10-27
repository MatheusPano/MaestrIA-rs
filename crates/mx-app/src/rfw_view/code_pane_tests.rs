use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext, px, size};
use mx_theme::{Typography, default_theme};
use serde_json::{Value, json};

use super::code_editor::find_all;
use super::code_pane::{CHANGE_DELAY, CURSOR_DELAY, CodePane};
use super::view::RfwView;
use super::{ActionSink, ViewAction};
use crate::setup::editor::TextEditor;

type Log = Rc<RefCell<Vec<ViewAction>>>;

const DOC: &str = "/proj/src/hello.js";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/../mx-rfw/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn data(file: Value) -> serde_json::Map<String, Value> {
    let mut d: serde_json::Map<String, Value> = serde_json::from_str(&fixture("data/editor.json")).unwrap();
    d.insert("file".into(), file);
    d
}

fn file(text: &str, rev: i64, jump: i64) -> Value {
    json!({ "path": DOC, "text": text, "rev": rev, "language": "javascript", "jump": jump, "readOnly": false })
}

fn open<'a>(cx: &'a mut TestAppContext, text: &str) -> (Entity<RfwView>, Log, &'a mut VisualTestContext) {
    let log: Log = Rc::default();
    let l = log.clone();
    let sink: ActionSink = Rc::new(move |a, _, _| l.borrow_mut().push(a));
    let d = data(file(text, 1, 0));
    let (view, vcx) = cx.add_window_view(move |_, _| {
        let mut v = RfwView::new(default_theme().clone(), &Typography::standard(), None, sink);
        v.set_library(&fixture("maestria-editor__ui__editor.rfwtxt"));
        v.set_root("root");
        v.merge_data(&d);
        v
    });
    vcx.simulate_resize(size(px(900.0), px(600.0)));
    vcx.run_until_parked();
    (view, log, vcx)
}

fn pane(view: &Entity<RfwView>, cx: &mut VisualTestContext) -> Entity<CodePane> {
    view.read_with(cx, |v, _| v.code_pane(DOC)).expect("no CodeEditor pane")
}

fn editor(view: &Entity<RfwView>, cx: &mut VisualTestContext) -> Entity<TextEditor> {
    let p = pane(view, cx);
    p.read_with(cx, |p, _| p.editor().clone())
}

fn text(view: &Entity<RfwView>, cx: &mut VisualTestContext) -> String {
    let e = editor(view, cx);
    e.read_with(cx, |e, _| e.text().to_owned())
}

fn actions(log: &Log, name: &str) -> Vec<serde_json::Map<String, Value>> {
    log.borrow().iter().filter(|a| a.action == name).map(|a| a.values.clone()).collect()
}

fn update(view: &Entity<RfwView>, cx: &mut VisualTestContext, d: Value) {
    view.update(cx, |v, cx| {
        v.merge_data(d.as_object().unwrap());
        cx.notify();
    });
    cx.run_until_parked();
}

#[gpui::test]
fn the_editor_shows_the_file_and_has_the_focus(cx: &mut TestAppContext) {
    let (view, _, cx) = open(cx, "const a = 1;\n");
    assert_eq!(text(&view, cx), "const a = 1;\n");
    let e = editor(&view, cx);
    assert!(cx.update(|window, cx| e.read(cx).is_focused(window)));
}

#[gpui::test]
fn typing_sends_change_once_after_the_pause_with_the_doc_and_text(cx: &mut TestAppContext) {
    let (view, log, cx) = open(cx, "abc");
    cx.simulate_keystrokes("end");
    cx.simulate_input("de");
    assert!(actions(&log, "change").is_empty());
    cx.executor().advance_clock(CHANGE_DELAY);
    cx.run_until_parked();
    let changes = actions(&log, "change");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].get("doc"), Some(&json!(DOC)));
    assert_eq!(changes[0].get("text"), Some(&json!("abcde")));
    assert_eq!(text(&view, cx), "abcde");
}

#[gpui::test]
fn ctrl_s_sends_save_with_the_typed_text(cx: &mut TestAppContext) {
    let (_, log, cx) = open(cx, "x");
    cx.simulate_keystrokes("end");
    cx.simulate_input("y");
    cx.simulate_keystrokes("ctrl-s");
    let saves = actions(&log, "save");
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].get("text"), Some(&json!("xy")));
    assert_eq!(saves[0].get("doc"), Some(&json!(DOC)));
}

#[gpui::test]
fn moving_the_cursor_sends_line_column_and_selection(cx: &mut TestAppContext) {
    let (_, log, cx) = open(cx, "ab\ncdef");
    cx.simulate_keystrokes("down right shift-right");
    cx.executor().advance_clock(CURSOR_DELAY);
    cx.run_until_parked();
    let last = actions(&log, "cursor").pop().unwrap();
    assert_eq!(last.get("line"), Some(&json!(2)));
    assert_eq!(last.get("column"), Some(&json!(3)));
    assert_eq!(last.get("selected"), Some(&json!(1)));
}

#[gpui::test]
fn an_update_with_the_same_revision_keeps_what_was_typed(cx: &mut TestAppContext) {
    let (view, _, cx) = open(cx, "abc");
    cx.simulate_input("X");
    update(&view, cx, json!({ "file": file("abc", 1, 0), "mode": "editor" }));
    assert_eq!(text(&view, cx), "Xabc");
}

#[gpui::test]
fn a_new_revision_replaces_the_text(cx: &mut TestAppContext) {
    let (view, _, cx) = open(cx, "abc");
    cx.simulate_input("X");
    update(&view, cx, json!({ "file": file("do disco", 2, 0) }));
    assert_eq!(text(&view, cx), "do disco");
}

#[gpui::test]
fn a_new_jump_moves_the_cursor_to_the_line_and_column(cx: &mut TestAppContext) {
    let (view, _, cx) = open(cx, "um\ndois\ntres");
    let mut f = file("um\ndois\ntres", 1, 1);
    f["line"] = json!(3);
    f["column"] = json!(2);
    update(&view, cx, json!({ "file": f }));
    let e = editor(&view, cx);
    assert_eq!(e.read_with(cx, |e, _| e.cursor_position()), (3, 2, 0));
}

#[gpui::test]
fn read_only_ignores_typing(cx: &mut TestAppContext) {
    let (view, _, cx) = open(cx, "abc");
    let mut f = file("abc", 1, 0);
    f["readOnly"] = json!(true);
    update(&view, cx, json!({ "file": f }));
    cx.simulate_input("X");
    cx.simulate_keystrokes("backspace");
    assert_eq!(text(&view, cx), "abc");
}

#[gpui::test]
fn ctrl_f_finds_and_enter_walks_the_matches(cx: &mut TestAppContext) {
    let (view, _, cx) = open(cx, "foo bar foo baz foo");
    cx.simulate_keystrokes("ctrl-f");
    cx.simulate_input("foo");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    let e = editor(&view, cx);
    assert_eq!(e.read_with(cx, |e, _| e.selection()), 0..3);
    cx.simulate_keystrokes("enter");
    assert_eq!(e.read_with(cx, |e, _| e.selection()), 8..11);
    cx.simulate_keystrokes("shift-enter");
    assert_eq!(e.read_with(cx, |e, _| e.selection()), 0..3);
    cx.simulate_keystrokes("escape");
    assert!(cx.update(|window, cx| e.read(cx).is_focused(window)));
}

#[gpui::test]
fn ctrl_h_replaces_every_match(cx: &mut TestAppContext) {
    let (view, _, cx) = open(cx, "a-b-c");
    let p = pane(&view, cx);
    cx.simulate_keystrokes("ctrl-h");
    cx.simulate_input("-");
    cx.run_until_parked();
    p.update(cx, |p, cx| p.replace_every_with("+", cx));
    assert_eq!(text(&view, cx), "a+b+c");
}

#[test]
fn find_ignores_case_until_aa_is_on_and_takes_a_regex_with_dot_star() {
    assert_eq!(find_all("Foo foo", "foo", false, false), [0..3, 4..7]);
    assert_eq!(find_all("Foo foo", "foo", true, false), vec![(4..7)]);
    assert_eq!(find_all("a1 b22", r"\d+", false, true), [1..2, 4..6]);
    assert_eq!(find_all("a.b", ".", false, false), vec![(1..2)]);
    assert!(find_all("abc", "(", false, true).is_empty());
    assert!(find_all("abc", "", false, false).is_empty());
}
