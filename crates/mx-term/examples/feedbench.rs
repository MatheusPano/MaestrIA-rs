use std::time::Instant;

use mx_term::{GridSize, SCROLLBACK, TerminalSession};

fn main() {
    let data: Vec<u8> = b"y\r\n".iter().copied().cycle().take(50_000_000).collect();
    for (cols, rows) in [(29u16, 5u16), (80, 24), (200, 60)] {
        let s = TerminalSession::new(GridSize { cols, rows, cell_width: 8, cell_height: 16 }, Default::default(), SCROLLBACK);
        let t = Instant::now();
        for chunk in data.chunks(0x10000) {
            s.feed(chunk);
        }
        let dt = t.elapsed().as_secs_f64();
        println!("{cols}x{rows} scrollback {SCROLLBACK}: {:.1} MB/s", 50.0 / dt);
    }
    for sb in [0usize, 100, 1000, 4000, 16000] {
        let s = TerminalSession::new(GridSize { cols: 29, rows: 5, cell_width: 8, cell_height: 16 }, Default::default(), sb);
        let t = Instant::now();
        for chunk in data[..10_000_000].chunks(0x10000) {
            s.feed(chunk);
        }
        println!("29x5 scrollback {sb}: {:.1} MB/s", 10.0 / t.elapsed().as_secs_f64());
    }
}
