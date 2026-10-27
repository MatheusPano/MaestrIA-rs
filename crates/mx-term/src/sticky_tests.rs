use std::time::{Duration, Instant};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;

use super::*;
use crate::reflow::selection_text_of;
use crate::session::GridSize;

struct Rig {
    term: Term<VoidListener>,
    parser: Processor,
    sticky: Sticky,
    now: Instant,
}

impl Rig {
    fn new(rows: u16) -> Self {
        let size = GridSize { cols: 80, rows, cell_width: 8, cell_height: 16 };
        let term = Term::new(Config::default(), &size, VoidListener);
        Self { term, parser: Processor::new(), sticky: Sticky::default(), now: Instant::now() }
    }

    fn write(&mut self, bytes: &str) {
        self.parser.advance(&mut self.term, bytes.as_bytes());
        self.sticky.written(self.now);
    }

    fn select(&mut self, row: usize, to: usize) {
        let last = self.term.columns() - 1;
        select(&mut self.term, CellPos::new(0, row), CellPos::new(last, to));
        self.sticky.selected(&self.term);
    }

    fn pump(&mut self, ms: u64) {
        let until = self.now + Duration::from_millis(ms);
        while let Some(d) = self.sticky.deadline().filter(|d| *d <= until) {
            self.sticky.fire(&mut self.term, d);
        }
        self.now = until;
    }

    fn selected(&self) -> Option<String> {
        selection_text_of(&self.term, false)
    }

    fn begin_end(&self) -> Option<(usize, usize)> {
        current(&self.term).map(|(b, e)| (b.y, e.y))
    }
}

fn frame(first: i32, rows: i32) -> String {
    (0..rows).map(|y| format!("\x1b[{};1Hmsg {} do transcript\x1b[K", y + 1, first + y)).collect()
}

fn alt(first: i32, rows: u16) -> Rig {
    let mut rig = Rig::new(rows);
    rig.write(&format!("\x1b[?1049h{}", frame(first, i32::from(rows.min(20)))));
    rig
}

#[test]
fn the_selection_follows_the_text_when_the_program_repaints() {
    let mut rig = alt(0, 24);
    rig.select(5, 5);
    assert_eq!(rig.selected().as_deref(), Some("msg 5 do transcript"));
    rig.write(&frame(3, 20));
    rig.pump(50);
    assert_eq!(rig.selected().as_deref(), Some("msg 5 do transcript"));
    assert_eq!(rig.begin_end().map(|b| b.0), Some(2));
    rig.write(&frame(-4, 20));
    rig.pump(50);
    assert_eq!(rig.selected().as_deref(), Some("msg 5 do transcript"));
    assert_eq!(rig.begin_end().map(|b| b.0), Some(9));
}

#[test]
fn a_skipped_space_and_a_written_space_are_the_same_text() {
    let mut rig = alt(0, 24);
    rig.write("\x1b[6;1H\x1b[2Kuma\x1b[6;5Hfrase\x1b[6;11Hcom\x1b[6;15Hburacos\x1b[K");
    rig.select(5, 5);
    rig.write(&frame(3, 20));
    rig.write("\x1b[3;1Huma frase com buracos\x1b[K");
    rig.pump(50);
    assert_eq!(rig.begin_end().map(|b| b.0), Some(2));
    assert_eq!(rig.selected().as_deref(), Some("uma frase com buracos"));
}

#[test]
fn several_rows_move_together() {
    let mut rig = alt(0, 24);
    rig.select(4, 6);
    let before = rig.selected();
    rig.write(&frame(2, 20));
    rig.pump(50);
    assert_eq!(rig.selected(), before);
    assert_eq!(rig.begin_end(), Some((2, 4)));
}

#[test]
fn and_also_when_it_scrolls_with_a_scroll_sequence() {
    let mut rig = Rig::new(24);
    rig.write(&format!("\x1b[?1049h{}", frame(0, 24)));
    rig.select(10, 10);
    rig.write("\x1b[3S");
    rig.pump(50);
    assert_eq!(rig.selected().as_deref(), Some("msg 10 do transcript"));
    assert_eq!(rig.begin_end().map(|b| b.0), Some(7));
}

#[test]
fn a_frame_that_arrives_in_two_pieces_does_not_drop_the_selection() {
    let mut rig = alt(0, 24);
    rig.select(12, 12);
    let next = frame(3, 20);
    let half = next.find("\x1b[6;1H").unwrap();
    rig.write(&format!("\x1b[2J{}", &next[..half]));
    rig.pump(50);
    rig.write(&next[half..]);
    rig.pump(300);
    assert_eq!(rig.selected().as_deref(), Some("msg 12 do transcript"));
    assert_eq!(rig.begin_end().map(|b| b.0), Some(9));
}

#[test]
fn text_that_left_the_screen_takes_the_selection_with_it() {
    let mut rig = alt(0, 24);
    rig.select(1, 1);
    rig.write(&frame(10, 20));
    rig.pump(300);
    assert_eq!(rig.begin_end(), None);
}

#[test]
fn on_the_main_screen_the_selection_stays_where_it_is() {
    let mut rig = Rig::new(24);
    for i in 0..10 {
        rig.write(&format!("linha {i}\r\n"));
    }
    rig.select(3, 3);
    rig.write("\x1b[4;1Houtra coisa\x1b[K");
    rig.pump(300);
    assert_eq!(rig.begin_end().map(|b| b.0), Some(3));
}

#[test]
fn clearing_the_selection_stops_following() {
    let mut rig = alt(0, 24);
    rig.select(5, 5);
    rig.term.selection = None;
    rig.sticky.selected(&rig.term);
    rig.write(&frame(3, 20));
    rig.pump(300);
    assert_eq!(rig.begin_end(), None);
}

#[test]
fn leaving_the_alternate_screen_forgets_the_selection() {
    let mut rig = alt(0, 24);
    rig.select(5, 5);
    rig.write("\x1b[?1049l");
    rig.pump(300);
    assert_eq!(rig.begin_end(), None);
}

#[test]
fn the_smallest_shift_wins_and_down_before_up() {
    let screen: Vec<String> = ["a", "x", "a", "x", "a"].iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(find_shift(&["a".into()], 2, &screen), Some(0));
    assert_eq!(find_shift(&["x".into()], 2, &screen), Some(1));
    assert_eq!(find_shift(&["z".into()], 2, &screen), None);
}
