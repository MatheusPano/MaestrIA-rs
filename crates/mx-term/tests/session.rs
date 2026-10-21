use std::time::{Duration, Instant};

use mx_term::{GridSize, SpawnOptions, TermEvent, TerminalSession};
use mx_theme::TerminalPalette;

const SIZE: GridSize = GridSize {
    cols: 40,
    rows: 10,
    cell_width: 8,
    cell_height: 16,
};

fn row_text(session: &TerminalSession, row: usize) -> String {
    session
        .snapshot()
        .row(row)
        .iter()
        .map(|c| c.c)
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[test]
fn runs_program_and_reports_exit() {
    let options = SpawnOptions {
        program: Some("/bin/sh".into()),
        args: vec![
            "-c".into(),
            "printf 'hello\\033[1mbold'; sleep 0.2; exit 3".into(),
        ],
        ..Default::default()
    };
    let (session, events) =
        TerminalSession::spawn(options, SIZE, TerminalPalette::default()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut exit = None;
    while Instant::now() < deadline && exit.is_none() {
        if let Ok(TermEvent::Exited(code)) = events.recv_blocking() {
            exit = Some(code);
        }
    }
    assert_eq!(exit, Some(Some(3)));
    assert_eq!(row_text(&session, 0), "hellobold");
    let snap = session.snapshot();
    assert!(!snap.row(0)[0].style.bold);
    assert!(snap.row(0)[5].style.bold);
}

#[test]
fn echoes_written_input_and_resizes() {
    let options = SpawnOptions {
        program: Some("/bin/cat".into()),
        ..Default::default()
    };
    let (session, events) =
        TerminalSession::spawn(options, SIZE, TerminalPalette::default()).unwrap();
    session.write(b"ping\r".as_slice());
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !row_text(&session, 1).contains("ping") {
        let _ = events.recv_blocking();
    }
    assert_eq!(row_text(&session, 0), "ping");
    assert_eq!(row_text(&session, 1), "ping");
    session.resize(GridSize {
        cols: 60,
        rows: 20,
        ..SIZE
    });
    let snap = session.snapshot();
    assert_eq!((snap.cols, snap.rows), (60, 20));
}
