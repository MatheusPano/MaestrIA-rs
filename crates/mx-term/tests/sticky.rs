use std::time::{Duration, Instant};

use mx_term::{GridSize, SelectionKind, TerminalSession};
use mx_theme::TerminalPalette;

fn frame(first: i32) -> String {
    (0..20).map(|y| format!("\x1b[{};1Hmsg {} do transcript\x1b[K", y + 1, first + y)).collect()
}

fn until(mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    f()
}

fn row_selected(t: &TerminalSession, row: usize) -> bool {
    t.snapshot().row(row).iter().take(19).all(|c| c.selected)
}

#[test]
fn a_selection_on_the_alternate_screen_follows_its_text_through_the_session() {
    let size = GridSize { cols: 80, rows: 24, cell_width: 8, cell_height: 16 };
    let t = TerminalSession::new(size, TerminalPalette::default(), 100);
    t.feed(format!("\x1b[?1049h{}", frame(0)).as_bytes());
    t.start_selection(0, 5, false, SelectionKind::Cells);
    t.update_selection(79, 5, true);
    t.feed(frame(3).as_bytes());
    assert!(until(|| row_selected(&t, 2)));
    assert_eq!(t.selected_text().as_deref(), Some("msg 5 do transcript"));
    t.clear_selection();
    t.feed(frame(0).as_bytes());
    std::thread::sleep(Duration::from_millis(200));
    assert!(!t.has_selection());
}
