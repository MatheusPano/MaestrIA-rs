use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mx_core::ports::{Term, TermFactory};
use mx_term::{GridSize, PasteOutcome, SpawnOptions, TermEvent, TermRegistry, TerminalSession};
use mx_theme::TerminalPalette;

const SIZE: GridSize = GridSize { cols: 40, rows: 10, cell_width: 8, cell_height: 16 };

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

fn sh(script: &str) -> TerminalSession {
    let options = SpawnOptions {
        program: Some("/bin/sh".into()),
        args: vec!["-c".into(), script.into()],
        ..Default::default()
    };
    TerminalSession::spawn(options, SIZE, TerminalPalette::default()).unwrap()
}

fn capture(dir: &Path, bytes: usize) -> (TerminalSession, std::path::PathBuf) {
    let file = dir.join("in.bin");
    let script = format!(
        "stty raw -echo; printf ready; head -c {bytes} > '{}'; sleep 5",
        file.display()
    );
    let t = sh(&script);
    assert!(until(Duration::from_secs(5), || t.row_text(0).contains("ready")));
    (t, file)
}

fn read_when(file: &Path, len: usize) -> Vec<u8> {
    until(Duration::from_secs(5), || std::fs::metadata(file).is_ok_and(|m| m.len() as usize >= len));
    std::fs::read(file).unwrap_or_default()
}

#[test]
fn runs_program_and_reports_exit() {
    let t = sh("printf 'hello\\033[1mbold'; sleep 0.2; exit 3");
    let events = t.events();
    let mut exit = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && exit.is_none() {
        if let Ok(TermEvent::Exited(code)) = events.recv_blocking() {
            exit = Some(code);
        }
    }
    assert_eq!(exit, Some(Some(3)));
    assert_eq!(t.row_text(0), "hellobold");
    assert!((0..4).any(|r| t.row_text(r).contains("[processo saiu com 3]")));
    let snap = t.snapshot();
    assert!(!snap.row(0)[0].style.bold);
    assert!(snap.row(0)[5].style.bold);
    assert!(t.is_exited());
    assert_eq!(t.exit_status(), Some(3));
    assert_eq!(t.child_pid(), None);
}

#[test]
fn echoes_written_input_and_resizes() {
    let t = sh("exec cat");
    t.write(b"ping\r");
    assert!(until(Duration::from_secs(5), || t.row_text(1).contains("ping")));
    assert_eq!(t.row_text(0), "ping");
    t.resize(GridSize { cols: 60, rows: 20, ..SIZE });
    let snap = t.snapshot();
    assert_eq!((snap.cols, snap.rows), (60, 20));
}

#[test]
fn pty_size_follows_resize() {
    let t = sh("sleep 0.3; stty size; sleep 2");
    t.resize(GridSize { cols: 83, rows: 22, ..SIZE });
    assert!(until(Duration::from_secs(5), || t.row_text(0) == "22 83"));
}

#[test]
fn paste_writes_clipboard_text() {
    let dir = tempfile::tempdir().unwrap();
    let (t, file) = capture(dir.path(), 10);
    assert_eq!(t.paste_clipboard(Some("git status"), true), PasteOutcome::Text);
    assert_eq!(read_when(&file, 10), b"git status");
}

#[test]
fn paste_without_text_sends_ctrl_v_for_claude() {
    let dir = tempfile::tempdir().unwrap();
    let (t, file) = capture(dir.path(), 1);
    assert_eq!(t.paste_clipboard(None, true), PasteOutcome::CtrlV);
    assert_eq!(read_when(&file, 1), b"\x16");
}

#[test]
fn paste_without_text_in_shell_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (t, file) = capture(dir.path(), 1);
    assert_eq!(t.paste_clipboard(Some(""), false), PasteOutcome::Nothing);
    t.write(b"!");
    assert_eq!(read_when(&file, 1), b"!");
}

#[test]
fn bracketed_paste_wraps_text() {
    let dir = tempfile::tempdir().unwrap();
    let (t, file) = capture(dir.path(), 15);
    t.feed(b"\x1b[?2004h");
    t.paste("a\nb");
    assert_eq!(read_when(&file, 15), b"\x1b[200~a\rb\x1b[201~");
}

#[test]
fn bracketed_paste_strips_escape_so_the_clipboard_cannot_end_the_paste() {
    let dir = tempfile::tempdir().unwrap();
    let (t, file) = capture(dir.path(), 27);
    t.feed(b"\x1b[?2004h");
    t.paste("x\x1b[201~rm -rf ~\r");
    assert_eq!(read_when(&file, 27), b"\x1b[200~x[201~rm -rf ~\r\x1b[201~");
}

#[test]
fn submit_strips_escape_from_the_text() {
    let dir = tempfile::tempdir().unwrap();
    let (mut t, file) = capture(dir.path(), 5);
    Term::submit(&mut t, "\x1b[Aok");
    assert_eq!(read_when(&file, 5), b"[Aok\r");
}

#[test]
fn wheel_in_alt_screen_with_mouse_reports_sgr() {
    let dir = tempfile::tempdir().unwrap();
    let (t, file) = capture(dir.path(), 21);
    t.feed(b"\x1b[?1049h\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h");
    t.wheel(true, 0, 0);
    t.wheel(false, 39, 7);
    assert_eq!(read_when(&file, 21), b"\x1b[<64;1;1M\x1b[<65;40;8M");
}

#[test]
fn shift_enter_newline_sends_esc_cr() {
    let dir = tempfile::tempdir().unwrap();
    let (t, file) = capture(dir.path(), 2);
    t.newline();
    assert_eq!(read_when(&file, 2), b"\x1b\r");
}

#[test]
fn submit_pastes_then_presses_enter() {
    let dir = tempfile::tempdir().unwrap();
    let (mut t, file) = capture(dir.path(), 4);
    Term::submit(&mut t, "oi!");
    assert_eq!(read_when(&file, 4), b"oi!\r");
}

#[test]
fn start_in_missing_cwd_reports_exit_minus_one() {
    let registry = TermRegistry::new(TerminalPalette::default());
    let exits = Arc::new(Mutex::new(Vec::new()));
    let seen = exits.clone();
    registry.on_exit(Arc::new(move |tab: &str, code| seen.lock().unwrap().push((tab.to_owned(), code))));
    let mut term = registry.create("t1");
    term.start_shell("/nao/existe/mesmo");
    assert!(term.exited());
    assert_eq!(term.exit_code(), Some(-1));
    assert_eq!(*exits.lock().unwrap(), [("t1".to_owned(), Some(-1))]);
    let session = registry.get("t1").unwrap();
    assert!(session.row_text(0).contains("a pasta /nao/existe/mesmo não existe"));
}

#[test]
fn command_runs_in_login_shell_with_exec_and_note() {
    let dir = tempfile::tempdir().unwrap();
    let registry = TermRegistry::new(TerminalPalette::default());
    let mut term = registry.create("t2");
    term.start_command("sh -c 'printf \"$TERM:$FORCE_HYPERLINK:$PWD\"; exit 7'", dir.path().to_str().unwrap(), Some("mostrar"));
    let session = registry.get("t2").unwrap();
    assert!(until(Duration::from_secs(10), || term.exited()));
    assert_eq!(term.exit_code(), Some(7));
    assert_eq!(session.row_text(0), "$ mostrar");
    let out = (1..4).map(|r| session.row_text(r)).collect::<Vec<_>>().join("");
    assert!(out.contains("xterm-256color:1:"), "{out}");
}

#[test]
fn relaunch_only_after_exit_keeps_scrollback() {
    let registry = TermRegistry::new(TerminalPalette::default());
    let mut term = registry.create("t3");
    term.start_command("printf um", "/", None);
    assert!(until(Duration::from_secs(10), || term.exited()));
    term.relaunch("printf dois", "/");
    assert!(until(Duration::from_secs(10), || term.exited()));
    let s = registry.get("t3").unwrap();
    let text = (0..10).map(|r| s.row_text(r)).collect::<Vec<_>>().join("\n");
    assert!(text.contains("um") && text.contains("dois"), "{text}");
}

#[test]
fn start_command_on_exited_term_restarts_in_place() {
    let registry = TermRegistry::new(TerminalPalette::default());
    let mut term = registry.create("t8");
    term.start_command("sh -c 'exit 4'", "/", None);
    assert!(until(Duration::from_secs(10), || term.exited()));
    assert_eq!(term.exit_code(), Some(4));
    term.start_command("sleep 5", "/", None);
    assert!(!term.exited());
    assert!(term.pid().is_some());
    futures_block(term.kill());
}

#[test]
fn park_only_before_first_start() {
    let registry = TermRegistry::new(TerminalPalette::default());
    let mut term = registry.create("t4");
    term.park();
    assert!(term.exited());
    assert_eq!(term.exit_code(), None);
    assert_eq!(term.pid(), None);
}

#[test]
fn kill_hangs_up_the_process_group() {
    let registry = TermRegistry::new(TerminalPalette::default());
    let mut term = registry.create("t5");
    term.start_command("sleep 30", "/", None);
    assert!(term.pid().is_some());
    let started = Instant::now();
    futures_block(term.kill());
    assert!(until(Duration::from_secs(2), || term.exited()));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn kill_escalates_to_sigkill_after_grace() {
    let registry = TermRegistry::new(TerminalPalette::default());
    let mut term = registry.create("t6");
    term.start_command("sh -c \"trap '' HUP; printf armed; while :; do sleep 0.1; done\"", "/", None);
    let s = registry.get("t6").unwrap();
    assert!(until(Duration::from_secs(10), || s.row_text(1).contains("armed")));
    let started = Instant::now();
    futures_block(term.kill());
    assert!(until(Duration::from_secs(2), || term.exited()));
    assert!(started.elapsed() >= Duration::from_millis(2900));
}

#[test]
fn registry_forgets_dropped_sessions() {
    let registry = TermRegistry::new(TerminalPalette::default());
    let term = registry.create("t7");
    assert!(registry.get("t7").is_some());
    drop(term);
    assert!(registry.get("t7").is_none());
}

#[test]
fn registry_sessions_know_their_tab() {
    let registry = TermRegistry::new(TerminalPalette::default());
    let _term = registry.create("t8");
    assert_eq!(registry.get("t8").unwrap().tab_id().as_deref(), Some("t8"));
    assert_eq!(TerminalSession::with_palette(TerminalPalette::default()).tab_id(), None);
}

fn futures_block(fut: mx_core::ports::BoxFuture<'static, ()>) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        let mut fut = fut;
        loop {
            if fut.as_mut().poll(&mut cx).is_ready() {
                let _ = tx.send(());
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
}
