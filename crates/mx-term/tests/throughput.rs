use std::time::{Duration, Instant};

use mx_term::{GridSize, SpawnOptions, TerminalSession};
use mx_theme::TerminalPalette;

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
