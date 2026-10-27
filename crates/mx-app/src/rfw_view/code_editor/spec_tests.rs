use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window,
    div, px, size,
};
use mx_theme::{Typography, default_theme};
use serde_json::{Value, json};

use super::{own_code_buffers_with, owners, syntax_colors};
use crate::rfw_view::code_pane::{CHANGE_DELAY, CURSOR_DELAY, CodePane};
use crate::rfw_view::style::hsla;
use crate::rfw_view::view::RfwView;
use crate::rfw_view::{ActionSink, ViewAction};
use crate::setup::editor::{TextEditor, comment_style};

const LIBRARY: &str = r#"
import core.widgets;
import maestria;
widget root = Column(children: [
  Text(text: data.file.path),
  Expanded(child: CodeEditor(
    doc: data.file.path,
    text: data.file.text,
    revision: data.file.rev,
    language: "dart",
    line: data.file.line,
    jump: data.file.jump,
    lineNumbers: data.opts.lineNumbers,
    autofocus: data.opts.autofocus,
    hint: data.opts.hint,
    onChange: event "change" {},
    onSave: event "save" {},
    onCursor: event "cursor" {},
  )),
]);
"#;

type Log = Rc<RefCell<Vec<ViewAction>>>;

struct Host {
    view: Option<Entity<RfwView>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(self.view.clone())
    }
}

fn rfw(log: &Log, data: &Value, cx: &mut gpui::App) -> Entity<RfwView> {
    let l = log.clone();
    let sink: ActionSink = Rc::new(move |a, _, _| l.borrow_mut().push(a));
    let data = data.as_object().unwrap().clone();
    let view = cx.new(|_| {
        let mut v = RfwView::new(default_theme().clone(), &Typography::standard(), None, sink);
        v.set_library(LIBRARY);
        v.set_root("root");
        v.merge_data(&data);
        v
    });
    own_code_buffers_with(view.entity_id(), "tab1", Rc::new(|_, _| true), cx);
    view
}

fn file(path: &str, text: &str, rev: i64) -> Value {
    json!({ "file": { "path": path, "text": text, "rev": rev }, "opts": {} })
}

fn mount(cx: &mut TestAppContext, data: Value) -> (Entity<Host>, Log, &mut VisualTestContext) {
    let log: Log = Rc::default();
    let l = log.clone();
    let (host, vcx) = cx.add_window_view(move |_, cx| Host { view: Some(rfw(&l, &data, cx)) });
    vcx.simulate_resize(size(px(700.0), px(500.0)));
    vcx.run_until_parked();
    (host, log, vcx)
}

fn view(host: &Entity<Host>, cx: &mut VisualTestContext) -> Entity<RfwView> {
    host.read_with(cx, |h, _| h.view.clone()).expect("unmounted")
}

fn pane(host: &Entity<Host>, doc: &str, cx: &mut VisualTestContext) -> Entity<CodePane> {
    let v = view(host, cx);
    v.read_with(cx, |v, _| v.code_pane(doc)).expect("no pane for doc")
}

fn editor(host: &Entity<Host>, doc: &str, cx: &mut VisualTestContext) -> Entity<TextEditor> {
    let p = pane(host, doc, cx);
    p.read_with(cx, |p, _| p.editor().clone())
}

fn text(host: &Entity<Host>, doc: &str, cx: &mut VisualTestContext) -> String {
    let e = editor(host, doc, cx);
    e.read_with(cx, |e, _| e.text().to_owned())
}

fn set(host: &Entity<Host>, cx: &mut VisualTestContext, data: Value) {
    let v = view(host, cx);
    v.update(cx, |v, cx| {
        v.merge_data(data.as_object().unwrap());
        cx.notify();
    });
    cx.run_until_parked();
}

fn named(log: &Log, name: &str) -> Vec<serde_json::Map<String, Value>> {
    log.borrow().iter().filter(|a| a.action == name).map(|a| a.values.clone()).collect()
}

fn type_at_start(host: &Entity<Host>, doc: &str, what: &str, cx: &mut VisualTestContext) {
    let e = editor(host, doc, cx);
    e.update(cx, |e, cx| {
        e.place(0, 0, false, cx);
        e.insert(what, cx);
    });
}

#[gpui::test]
fn typing_sends_the_text_after_the_pause_and_save_sends_it_at_once(cx: &mut TestAppContext) {
    let (host, log, cx) = mount(cx, file("/repo/a.dart", "void main() {}\n", 1));
    assert_eq!(text(&host, "/repo/a.dart", cx), "void main() {}\n");
    assert!(named(&log, "change").is_empty());
    type_at_start(&host, "/repo/a.dart", "// oi\n", cx);
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    assert!(named(&log, "change").is_empty());
    cx.executor().advance_clock(std::time::Duration::from_millis(400));
    cx.run_until_parked();
    let changes = named(&log, "change");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["doc"], json!("/repo/a.dart"));
    assert_eq!(changes[0]["text"], json!("// oi\nvoid main() {}\n"));
    cx.simulate_keystrokes("ctrl-s");
    let saves = named(&log, "save");
    assert_eq!(saves.last().unwrap()["text"], json!("// oi\nvoid main() {}\n"));
    assert_eq!(named(&log, "change").len(), 1);
}

#[gpui::test]
fn the_buffer_stays_with_the_window_when_the_pane_leaves_and_comes_back(cx: &mut TestAppContext) {
    let (host, log, cx) = mount(cx, file("/repo/a.dart", "a\n", 1));
    let e = editor(&host, "/repo/a.dart", cx);
    e.update(cx, |e, cx| {
        e.select_everything(cx);
        e.insert("a\nb\n", cx);
    });
    host.update(cx, |h, cx| {
        h.view = None;
        cx.notify();
    });
    cx.run_until_parked();
    let changes = named(&log, "change");
    assert_eq!(changes.len(), 1, "unmounting does not wait for the debounce");
    assert_eq!(changes[0]["text"], json!("a\nb\n"));
    let l = log.clone();
    host.update(cx, |h, cx| {
        h.view = Some(rfw(&l, &file("/repo/a.dart", "a\n", 1), cx));
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(text(&host, "/repo/a.dart", cx), "a\nb\n");

    set(&host, cx, file("/repo/b.dart", "b\n", 1));
    assert_eq!(text(&host, "/repo/b.dart", cx), "b\n");
    set(&host, cx, file("/repo/a.dart", "a\n", 1));
    assert_eq!(text(&host, "/repo/a.dart", cx), "a\nb\n");
}

#[gpui::test]
fn switching_to_another_doc_sends_the_pending_change_at_once(cx: &mut TestAppContext) {
    let (host, log, cx) = mount(cx, file("/repo/a.dart", "a\n", 1));
    type_at_start(&host, "/repo/a.dart", "x", cx);
    set(&host, cx, file("/repo/b.dart", "b\n", 1));
    let changes = named(&log, "change");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["doc"], json!("/repo/a.dart"));
    assert_eq!(changes[0]["text"], json!("xa\n"));
    cx.executor().advance_clock(CHANGE_DELAY * 2);
    cx.run_until_parked();
    assert_eq!(named(&log, "change").len(), 1);
}

#[gpui::test]
fn a_new_revision_replaces_the_text_and_a_new_jump_moves_the_cursor(cx: &mut TestAppContext) {
    let (host, log, cx) = mount(cx, file("/repo/a.dart", "um\n", 1));
    set(
        &host,
        cx,
        json!({ "file": { "path": "/repo/a.dart", "text": "um\ndois\ntrês\n", "rev": 2, "line": 3, "jump": 1 }, "opts": {} }),
    );
    let e = editor(&host, "/repo/a.dart", cx);
    assert_eq!(e.read_with(cx, |e, _| e.text().to_owned()), "um\ndois\ntrês\n");
    assert_eq!(e.read_with(cx, |e, _| e.cursor_line()), 2);
    cx.executor().advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    assert!(named(&log, "change").is_empty(), "reloading is not editing");
    assert_eq!(named(&log, "cursor").last().unwrap()["line"], json!(3));
}

#[gpui::test]
fn reloading_keeps_the_cursor_on_its_line_at_column_one(cx: &mut TestAppContext) {
    let (host, _, cx) = mount(cx, file("/a.dart", "um\ndois\ntres\n", 1));
    let e = editor(&host, "/a.dart", cx);
    e.update(cx, |e, cx| e.place(1, 3, false, cx));
    set(&host, cx, file("/a.dart", "UM\nDOIS\nTRES\n", 2));
    assert_eq!(e.read_with(cx, |e, _| e.cursor_position()), (2, 1, 0));
}

#[gpui::test]
fn the_cursor_event_waits_for_the_selection_to_settle(cx: &mut TestAppContext) {
    let (_, log, cx) = mount(cx, file("/a.dart", "ab\ncd\n", 1));
    cx.simulate_keystrokes("right right");
    assert!(named(&log, "cursor").is_empty());
    cx.executor().advance_clock(CURSOR_DELAY);
    cx.run_until_parked();
    let cursors = named(&log, "cursor");
    assert_eq!(cursors.len(), 1);
    assert_eq!(cursors[0]["column"], json!(3));
}

#[gpui::test]
fn closing_the_tab_drops_its_buffers(cx: &mut TestAppContext) {
    let (host, _, cx) = mount(cx, file("/a.dart", "a", 1));
    cx.update(|_, cx| assert_eq!(owners(cx), ["tab1"]));
    let id = view(&host, cx).entity_id();
    cx.update(|_, cx| own_code_buffers_with(id, "tab1", Rc::new(|_, _| false), cx));
    host.update(cx, |h, cx| {
        h.view = None;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(owners(cx).is_empty()));
}

#[gpui::test]
fn line_numbers_autofocus_and_hint_follow_the_args(cx: &mut TestAppContext) {
    let data = json!({ "file": { "path": "/a", "text": "", "rev": 1 }, "opts": { "lineNumbers": false, "autofocus": false, "hint": "vazio" } });
    let (host, _, cx) = mount(cx, data);
    let e = editor(&host, "/a", cx);
    assert!(!cx.update(|window, cx| e.read(cx).is_focused(window)));
    let style = e.read_with(cx, |e, _| e.style().clone());
    assert!(style.gutter.is_none());
    assert_eq!(style.hint.map(|(h, _)| h).as_deref(), Some("vazio"));
    assert_eq!(style.language, "dart");
}

#[test]
fn syntax_colours_come_from_the_theme_terminal_palette() {
    let theme = default_theme();
    let c = syntax_colors(&theme);
    assert_eq!(c.keyword, hsla(theme.ansi.magenta));
    assert_eq!(c.string, hsla(theme.ansi.green));
    assert!(comment_style("dart").is_some());
    assert!(comment_style("klingon").is_none());
}
