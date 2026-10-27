use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mx_core::ports::{TermInput, TermKey};
use mx_term::{GridSize, SpawnOptions, TerminalSession};
use mx_theme::TerminalPalette;

const SIZE: GridSize = GridSize { cols: 60, rows: 10, cell_width: 8, cell_height: 16 };

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

fn capture(dir: &Path, bytes: usize, setup: &str) -> (TerminalSession, PathBuf) {
    let file = dir.join("in.bin");
    let script = format!("stty raw -echo; printf '{setup}ready'; head -c {bytes} > '{}'; sleep 5", file.display());
    let t = sh(&script);
    assert!(until(Duration::from_secs(5), || t.row_text(0).contains("ready")));
    (t, file)
}

fn read_when(file: &Path, len: usize) -> Vec<u8> {
    until(Duration::from_secs(5), || std::fs::metadata(file).is_ok_and(|m| m.len() as usize >= len));
    std::fs::read(file).unwrap_or_default()
}

const TRUST_SCREEN: &[u8] = b"Accessing workspace:\r\n\r\nQuick safety check: Is this a project you created or one you trust?\r\n\xe2\x9d\xaf No, exit\r\n  Yes, I trust this folder\r\n";

#[test]
fn the_trust_prompt_on_screen_is_detected_until_the_screen_is_cleared() {
    let t = TerminalSession::spawn(
        SpawnOptions { program: Some("/bin/sh".into()), args: vec!["-c".into(), "sleep 5".into()], ..Default::default() },
        SIZE,
        TerminalPalette::default(),
    )
    .unwrap();
    t.feed(TRUST_SCREEN);
    assert!(t.asks_trust());
    assert!(t.screen_text().contains("Yes, I trust this folder\n"));
    t.feed(b"\x1b[2J\x1b[H> ");
    assert!(!t.asks_trust());
    assert_eq!(t.screen_text().lines().count(), usize::from(SIZE.rows));
}

#[test]
fn send_keys_types_each_key_with_a_gap_and_returns_after_the_last() {
    let dir = tempfile::tempdir().unwrap();
    let (mut t, file) = capture(dir.path(), 7, "");
    let keys = vec![TermKey::Up, TermKey::Text("oi".into()), TermKey::Enter, TermKey::Backspace];
    let started = Instant::now();
    block_on(TermInput::send_keys(&mut t, keys));
    assert!(started.elapsed() >= Duration::from_millis(180));
    assert_eq!(read_when(&file, 7), b"\x1b[Aoi\r\x7f");
}

#[test]
fn send_keys_honours_the_application_cursor_mode() {
    let dir = tempfile::tempdir().unwrap();
    let (mut t, file) = capture(dir.path(), 6, "\\033[?1h");
    block_on(TermInput::send_keys(&mut t, vec![TermKey::Down, TermKey::Right]));
    assert_eq!(read_when(&file, 6), b"\x1bOB\x1bOC");
}

#[test]
fn send_keys_stops_quietly_once_the_session_exits() {
    let mut t = sh("exit 0");
    assert!(until(Duration::from_secs(5), || t.is_exited()));
    let started = Instant::now();
    block_on(TermInput::send_keys(&mut t, vec![TermKey::Enter; 5]));
    assert!(started.elapsed() < Duration::from_millis(600));
}

#[test]
fn enter_alone_sends_a_carriage_return() {
    let dir = tempfile::tempdir().unwrap();
    let (mut t, file) = capture(dir.path(), 1, "");
    TermInput::enter(&mut t);
    assert_eq!(read_when(&file, 1), b"\r");
}

fn block_on<F: std::future::Future<Output = ()>>(fut: F) {
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let mut fut = std::pin::pin!(fut);
    let deadline = Instant::now() + Duration::from_secs(10);
    while fut.as_mut().poll(&mut cx).is_pending() {
        assert!(Instant::now() < deadline, "send_keys never finished");
        std::thread::sleep(Duration::from_millis(5));
    }
}
