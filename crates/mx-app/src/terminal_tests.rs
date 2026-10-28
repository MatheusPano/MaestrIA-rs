use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{
    AppContext, Context, Entity, EntityInputHandler, Focusable, IntoElement, KeyDownEvent, Keystroke, Modifiers,
    MouseButton, ParentElement, Pixels, Point, Render, Styled, TestAppContext, VisualTestContext, Window, div, point,
};
use mx_term::{GridSize, SelectionKind, SpawnOptions, TerminalSession};
use mx_theme::TerminalPalette;

use super::TerminalView;

const SIZE: GridSize = GridSize { cols: 40, rows: 10, cell_width: 8, cell_height: 16 };

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("mx-term-view-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Host(Option<Entity<TerminalView>>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(self.0.clone())
    }
}

fn until(limit: Duration, mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    f()
}

fn capture(dir: &Path, bytes: usize) -> (TerminalSession, PathBuf) {
    let file = dir.join("in.bin");
    let script = format!("stty raw -echo; printf ready; head -c {bytes} > '{}'; sleep 5", file.display());
    let options = SpawnOptions { program: Some("/bin/sh".into()), args: vec!["-c".into(), script], ..Default::default() };
    let t = TerminalSession::spawn(options, SIZE, TerminalPalette::default()).unwrap();
    assert!(until(Duration::from_secs(5), || t.row_text(0).contains("ready")));
    (t, file)
}

fn settle(file: &Path, len: usize) -> Vec<u8> {
    until(Duration::from_secs(5), || std::fs::metadata(file).is_ok_and(|m| m.len() as usize >= len));
    std::thread::sleep(Duration::from_millis(200));
    std::fs::read(file).unwrap_or_default()
}

fn mount<'a>(cx: &'a mut TestAppContext, session: &TerminalSession) -> (Entity<Host>, Entity<TerminalView>, &'a mut VisualTestContext) {
    let s = session.clone();
    let (host, vcx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| TerminalView::for_session(s, &TerminalPalette::default(), window, cx));
        Host(Some(view))
    });
    vcx.run_until_parked();
    let view = host.read_with(vcx, |h, _| h.0.clone().unwrap());
    (host, view, vcx)
}

fn first_row(view: &Entity<TerminalView>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |v, _| {
        let st = v.render.borrow();
        st.snapshot().row(0).iter().map(|c| c.c).collect::<String>().trim_end().to_owned()
    })
}

#[gpui::test]
fn a_view_shown_again_repaints_on_new_output(cx: &mut TestAppContext) {
    let session = TerminalSession::new(SIZE, TerminalPalette::default(), 100);
    let (host, _old, vcx) = mount(cx, &session);
    let s = session.clone();
    host.update_in(vcx, |h, window, cx| {
        h.0 = Some(cx.new(|cx| TerminalView::for_session(s, &TerminalPalette::default(), window, cx)));
        cx.notify();
    });
    drop(_old);
    vcx.run_until_parked();
    let shown = host.read_with(vcx, |h, _| h.0.clone().unwrap());
    for word in ["one", "two"] {
        session.feed(format!("\r{word}").as_bytes());
        vcx.executor().advance_clock(Duration::from_millis(50));
        assert_eq!(first_row(&shown, vcx), word);
    }
}

#[gpui::test]
fn streaming_output_is_drawn_on_the_next_output_frame(cx: &mut TestAppContext) {
    let session = TerminalSession::new(SIZE, TerminalPalette::default(), 100);
    let (_host, view, vcx) = mount(cx, &session);
    vcx.executor().advance_clock(Duration::from_millis(200));
    session.feed(b"\rone");
    vcx.run_until_parked();
    assert_eq!(first_row(&view, vcx), "one");
    session.feed(b"\rtwo");
    vcx.run_until_parked();
    assert_eq!(first_row(&view, vcx), "one");
    vcx.executor().advance_clock(Duration::from_millis(50));
    assert_eq!(first_row(&view, vcx), "two");
}

#[gpui::test]
fn a_painted_frame_without_new_output_keeps_the_cached_snapshot(cx: &mut TestAppContext) {
    let session = TerminalSession::new(SIZE, TerminalPalette::default(), 100);
    let (_host, view, vcx) = mount(cx, &session);
    assert!(!view.read_with(vcx, |v, _| v.dirty.take()));
    session.feed(b"x");
    assert!(view.read_with(vcx, |v, _| v.dirty.take()));
}

#[gpui::test]
fn wayland_ime_commit_keys_are_deduped_against_real_keys_but_not_each_other(cx: &mut TestAppContext) {
    let dir = Scratch::new("ime");
    let (session, file) = capture(&dir.0, 3);
    let (_host, view, vcx) = mount(cx, &session);
    let key = |k: &str, ime_commit: bool| KeyDownEvent {
        keystroke: Keystroke { modifiers: Modifiers::default(), key: k.into(), key_char: Some(k.into()) },
        is_held: false,
        ime_commit,
    };
    view.update_in(vcx, |v, window, cx| {
        v.on_key_down(&key("a", false), window, cx);
        v.replace_text_in_range(None, "a", window, cx);
        v.on_key_down(&key("a", true), window, cx);
        v.replace_text_in_range(None, "a", window, cx);
        for _ in 0..2 {
            v.on_key_down(&key("l", true), window, cx);
            v.replace_text_in_range(None, "l", window, cx);
        }
    });
    assert_eq!(settle(&file, 3), b"all");
}

#[gpui::test]
fn dragging_inside_one_cell_reports_motion_once_per_cell(cx: &mut TestAppContext) {
    let dir = Scratch::new("drag");
    let (session, file) = capture(&dir.0, 28);
    session.feed(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");
    let (_host, view, vcx) = mount(cx, &session);
    let (origin, w, h) = view.read_with(vcx, |v, _| {
        let st = v.render.borrow();
        (st.origin, st.cell().width, st.cell().height)
    });
    let at = |col: f32, fx: f32| -> Point<Pixels> { point(origin.x + w * (col + fx), origin.y + h * 0.5) };
    vcx.simulate_mouse_down(at(0., 0.2), MouseButton::Left, Modifiers::none());
    for fx in [0.3, 0.5, 0.7, 0.9] {
        vcx.simulate_mouse_move(at(0., fx), MouseButton::Left, Modifiers::none());
    }
    vcx.simulate_mouse_move(at(1., 0.2), MouseButton::Left, Modifiers::none());
    vcx.simulate_mouse_move(at(1., 0.6), MouseButton::Left, Modifiers::none());
    vcx.simulate_mouse_up(at(1., 0.6), MouseButton::Left, Modifiers::none());
    assert_eq!(settle(&file, 28), b"\x1b[<0;1;1M\x1b[<32;2;1M\x1b[<0;2;1m");
}

const ERROS: &[u8] = b"primeiro erro\r\nnada aqui\r\nsegundo erro\r\n";

fn counter(view: &Entity<TerminalView>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |v, _| v.find.as_ref().and_then(|f| f.session.counter()))
}

fn highlights(view: &Entity<TerminalView>, cx: &mut VisualTestContext) -> usize {
    view.read_with(cx, |v, _| v.render.borrow().hits.len())
}

fn bar_focused(view: &Entity<TerminalView>, cx: &mut VisualTestContext) -> bool {
    view.update_in(cx, |v, window, cx| {
        v.find_bar.as_ref().is_some_and(|b| b.read(cx).focus_handle(cx).is_focused(window))
    })
}

fn open_find(view: &Entity<TerminalView>, cx: &mut VisualTestContext) {
    view.update_in(cx, |v, window, cx| v.open_find(window, cx));
    cx.run_until_parked();
}

fn search(view: &Entity<TerminalView>, cx: &mut VisualTestContext, query: &str) {
    open_find(view, cx);
    cx.simulate_input(query);
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.run_until_parked();
}

fn with_erros(cx: &mut TestAppContext) -> (TerminalSession, Entity<TerminalView>, &mut VisualTestContext) {
    let session = TerminalSession::new(SIZE, TerminalPalette::default(), 100);
    session.feed(ERROS);
    let (_host, view, vcx) = mount(cx, &session);
    (session, view, vcx)
}

#[gpui::test]
fn find_opens_one_bar_with_its_field_focused(cx: &mut TestAppContext) {
    let (_s, view, vcx) = with_erros(cx);
    view.update_in(vcx, |v, window, _| window.focus(&v.focus));
    open_find(&view, vcx);
    assert!(view.read_with(vcx, |v, _| v.find.is_some()));
    assert!(bar_focused(&view, vcx));
    assert_eq!(counter(&view, vcx), None);
}

#[gpui::test]
fn typing_highlights_every_hit_and_enter_walks_up_and_shift_enter_down(cx: &mut TestAppContext) {
    let (_s, view, vcx) = with_erros(cx);
    search(&view, vcx, "erro");
    assert_eq!(counter(&view, vcx).as_deref(), Some("2 de 2"));
    assert_eq!(highlights(&view, vcx), 2);
    vcx.simulate_keystrokes("enter");
    assert_eq!(counter(&view, vcx).as_deref(), Some("1 de 2"));
    vcx.simulate_keystrokes("shift-enter");
    assert_eq!(counter(&view, vcx).as_deref(), Some("2 de 2"));
}

#[gpui::test]
fn enter_while_the_query_is_pending_searches_first(cx: &mut TestAppContext) {
    let (_s, view, vcx) = with_erros(cx);
    open_find(&view, vcx);
    vcx.simulate_input("erro");
    vcx.simulate_keystrokes("enter");
    assert_eq!(counter(&view, vcx).as_deref(), Some("2 de 2"));
}

#[gpui::test]
fn a_query_with_no_hits_reads_nenhum_and_paints_nothing(cx: &mut TestAppContext) {
    let (_s, view, vcx) = with_erros(cx);
    search(&view, vcx, "inexistente");
    assert_eq!(counter(&view, vcx).as_deref(), Some("Nenhum"));
    assert_eq!(highlights(&view, vcx), 0);
}

#[gpui::test]
fn new_output_while_open_adds_hits_without_moving_the_current_one(cx: &mut TestAppContext) {
    let (session, view, vcx) = with_erros(cx);
    search(&view, vcx, "erro");
    session.feed(b"terceiro erro\r\n");
    vcx.run_until_parked();
    vcx.executor().advance_clock(Duration::from_millis(300));
    vcx.run_until_parked();
    assert_eq!(highlights(&view, vcx), 3);
    assert_eq!(counter(&view, vcx).as_deref(), Some("2 de 3"));
}

#[gpui::test]
fn escape_closes_the_bar_and_enter_goes_back_to_the_pty(cx: &mut TestAppContext) {
    let dir = Scratch::new("find-esc");
    let (session, file) = capture(&dir.0, 1);
    let (_host, view, vcx) = mount(cx, &session);
    search(&view, vcx, "erro");
    vcx.simulate_keystrokes("escape");
    assert!(!view.read_with(vcx, |v, _| v.find.is_some()));
    assert_eq!(highlights(&view, vcx), 0);
    vcx.simulate_keystrokes("enter");
    assert_eq!(settle(&file, 1), b"\r");
}

#[gpui::test]
fn opening_from_another_focused_field_moves_the_keyboard_to_the_bar(cx: &mut TestAppContext) {
    let (_s, view, vcx) = with_erros(cx);
    let other = vcx.update(|_, cx| cx.focus_handle());
    vcx.update(|window, _| window.focus(&other));
    open_find(&view, vcx);
    assert!(bar_focused(&view, vcx));
}

#[gpui::test]
fn reopening_keeps_the_last_query_selected_for_typing_over(cx: &mut TestAppContext) {
    let (_s, view, vcx) = with_erros(cx);
    search(&view, vcx, "nada");
    vcx.simulate_keystrokes("escape");
    open_find(&view, vcx);
    vcx.simulate_input("erro");
    vcx.executor().advance_clock(Duration::from_millis(150));
    vcx.run_until_parked();
    let text = view.read_with(vcx, |v, cx| v.find_bar.as_ref().unwrap().read(cx).text().to_owned());
    assert_eq!(text, "erro");
    assert_eq!(counter(&view, vcx).as_deref(), Some("2 de 2"));
}

#[gpui::test]
fn copy_in_a_claude_tab_joins_the_rows_claude_wrapped(cx: &mut TestAppContext) {
    let session = TerminalSession::new(SIZE, TerminalPalette::default(), 100);
    let (_host, view, vcx) = mount(cx, &session);
    let cols = session.size().cols as usize;
    let words = "Rivers shape landscapes and sustain".repeat(cols / 35 + 1);
    let first: String = words.chars().take(cols - 6).collect::<String>().trim_end().to_owned();
    session.feed(format!("\x1b[H\x1b[2J● {first}\r\n\x1b[2Cecosystems across the globe.").as_bytes());
    session.start_selection(0, 0, false, SelectionKind::Cells);
    session.update_selection(cols - 1, 1, true);
    for (claude, want) in [
        (true, format!("{first} ecosystems across the globe.")),
        (false, format!("● {first}\n  ecosystems across the globe.")),
    ] {
        view.update_in(vcx, |v, _, cx| {
            v.set_claude(claude);
            v.copy_selection(cx);
        });
        assert_eq!(vcx.read_from_clipboard().and_then(|c| c.text()), Some(want));
    }
}

#[test]
fn a_hit_split_by_the_wrap_paints_on_both_rows_and_off_screen_hits_paint_nothing() {
    use crate::terminal_element::{HitQuad, hit_quads};
    use mx_term::{Hit, Viewport};
    let view = Viewport { history: 5, display_offset: 0, rows: 4 };
    let marks = [(Hit::new((8, 5), (2, 6)), true), (Hit::new((0, 1), (3, 1)), false)];
    assert_eq!(
        hit_quads(&marks, view, 10),
        [
            HitQuad { row: 0, col: 8, len: 2, current: true },
            HitQuad { row: 1, col: 0, len: 2, current: true }
        ]
    );
}
