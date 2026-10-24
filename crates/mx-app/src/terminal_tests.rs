use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{
    AppContext, Context, Entity, EntityInputHandler, IntoElement, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    ParentElement, Pixels, Point, Render, Styled, TestAppContext, VisualTestContext, Window, div, point,
};
use mx_term::{GridSize, SpawnOptions, TerminalSession};
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
        vcx.run_until_parked();
        assert_eq!(first_row(&shown, vcx), word);
    }
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
