use std::time::Instant;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use mx_term::{GridSize, SCROLLBACK, TerminalSession};

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let data = std::fs::read(&path).unwrap();
    let mb = data.len() as f64 / 1e6;
    for (cols, rows) in [(80u16, 24u16), (200, 60)] {
        let size = GridSize { cols, rows, cell_width: 8, cell_height: 16 };
        for chunk in [0x10000usize, 2048, 160] {
            let mut term = Term::new(Config { scrolling_history: SCROLLBACK, ..Config::default() }, &size, VoidListener);
            let mut p: Processor = Processor::new();
            let t = Instant::now();
            for c in data.chunks(chunk) { p.advance(&mut term, c); }
            let raw = mb / t.elapsed().as_secs_f64();
            let s = TerminalSession::new(size, Default::default(), SCROLLBACK);
            let t = Instant::now();
            for c in data.chunks(chunk) { s.feed(c); }
            let feed = mb / t.elapsed().as_secs_f64();
            println!("{cols}x{rows} chunk {chunk}: alacritty {raw:.1} MB/s, feed {feed:.1} MB/s");
        }
    }
}
