use std::time::{Duration, Instant};

use mx_term::{GridSize, SpawnOptions, TermEvent, TerminalSession};
use mx_theme::TerminalPalette;

const WIDE: GridSize = GridSize { cols: 200, rows: 60, cell_width: 8, cell_height: 16 };

fn pending(t: &TerminalSession) -> Vec<TermEvent> {
    let events = t.events();
    std::iter::from_fn(|| events.try_recv().ok()).collect()
}

#[test]
fn a_flood_of_fed_output_wakes_the_view_once_per_frame() {
    let t = TerminalSession::new(WIDE, TerminalPalette::default(), 8000);
    let chunk = "y\r\n".repeat(1000);
    for _ in 0..500 {
        t.feed(chunk.as_bytes());
    }
    assert_eq!(pending(&t), [TermEvent::Wakeup]);
    assert!(t.take_dirty());
    for _ in 0..500 {
        t.feed(chunk.as_bytes());
    }
    assert_eq!(pending(&t), [TermEvent::Wakeup]);
    assert_eq!(t.row_text(59), "");
    assert_eq!(t.row_text(58), "y");
}

#[test]
fn a_pty_flood_wakes_the_view_once_until_the_frame_takes_it() {
    let options = SpawnOptions {
        program: Some("/bin/sh".into()),
        args: vec!["-c".into(), "yes | head -c 3000000".into()],
        ..Default::default()
    };
    let t = TerminalSession::spawn(options, WIDE, TerminalPalette::default()).unwrap();
    let started = Instant::now();
    while !t.is_exited() && started.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(t.is_exited());
    let events = pending(&t);
    assert_eq!(events.iter().filter(|e| **e == TermEvent::Wakeup).count(), 1, "{events:?}");
    assert!(events.contains(&TermEvent::Exited(Some(0))));
}

#[test]
#[ignore]
fn yes_50mb_throughput() {
    let options = SpawnOptions {
        program: Some("/bin/sh".into()),
        args: vec!["-c".into(), "yes | head -c 50000000; printf DONE".into()],
        ..Default::default()
    };
    let size = GridSize { cols: 100, rows: 40, cell_width: 8, cell_height: 16 };
    let started = Instant::now();
    let t = TerminalSession::spawn(options, size, TerminalPalette::default()).unwrap();
    while !t.is_exited() && started.elapsed() < Duration::from_secs(120) {
        std::thread::sleep(Duration::from_millis(5));
    }
    let secs = started.elapsed().as_secs_f64();
    eprintln!("50 MB in {secs:.2} s = {:.1} MB/s", 50.0 / secs);
    assert!(t.is_exited());
}

#[test]
#[ignore]
fn feed_50mb_without_pty() {
    let size = GridSize { cols: 100, rows: 40, cell_width: 8, cell_height: 16 };
    let t = TerminalSession::new(size, TerminalPalette::default(), 8000);
    let chunk = "y\r\n".repeat(2048);
    let started = Instant::now();
    for _ in 0..(50_000_000 / chunk.len()) {
        t.feed(chunk.as_bytes());
    }
    let secs = started.elapsed().as_secs_f64();
    eprintln!("feed: 50 MB in {secs:.2} s = {:.1} MB/s", 50.0 / secs);
}

#[test]
#[ignore]
fn raw_alacritty_50mb() {
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::{Config, Term};
    use alacritty_terminal::vte::ansi::Processor;
    let size = GridSize { cols: 100, rows: 40, cell_width: 8, cell_height: 16 };
    for history in [0usize, 8000] {
        let config = Config { scrolling_history: history, ..Config::default() };
        let mut term = Term::new(config, &size, VoidListener);
        let mut parser: Processor = Processor::new();
        let chunk = "y\r\n".repeat(2048);
        let started = Instant::now();
        for _ in 0..(10_000_000 / chunk.len()) {
            parser.advance(&mut term, chunk.as_bytes());
        }
        let secs = started.elapsed().as_secs_f64();
        eprintln!("raw history={history}: 10 MB in {secs:.2} s = {:.1} MB/s", 10.0 / secs);
    }
}
