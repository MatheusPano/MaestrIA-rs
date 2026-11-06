use std::fmt::Write as _;

use alacritty_terminal::grid::{Dimensions, GridCell};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags, Hyperlink};
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};

const STYLE_FLAGS: Flags = Flags::BOLD
    .union(Flags::DIM)
    .union(Flags::ITALIC)
    .union(Flags::ALL_UNDERLINES)
    .union(Flags::INVERSE)
    .union(Flags::HIDDEN)
    .union(Flags::STRIKEOUT);

#[derive(Clone, PartialEq)]
struct Style {
    fg: Color,
    bg: Color,
    flags: Flags,
    underline: Option<Color>,
}

impl Style {
    const PLAIN: Style = Style {
        fg: Color::Named(NamedColor::Foreground),
        bg: Color::Named(NamedColor::Background),
        flags: Flags::empty(),
        underline: None,
    };

    fn of(cell: &Cell) -> Style {
        Style { fg: cell.fg, bg: cell.bg, flags: cell.flags & STYLE_FLAGS, underline: cell.underline_color() }
    }
}

struct Pen {
    style: Style,
    link: Option<Hyperlink>,
}

impl Pen {
    fn new() -> Pen {
        Pen { style: Style::PLAIN, link: None }
    }

    fn take(&mut self, cell: &Cell, out: &mut String) {
        let style = Style::of(cell);
        if style != self.style {
            sgr(&style, out);
            self.style = style;
        }
        let link = cell.hyperlink();
        if link != self.link {
            match &link {
                Some(l) => {
                    let _ = write!(out, "\x1b]8;id={};{}\x1b\\", l.id(), l.uri());
                }
                None => out.push_str("\x1b]8;;\x1b\\"),
            }
            self.link = link;
        }
    }

    fn reset(&mut self, out: &mut String) {
        if self.link.take().is_some() {
            out.push_str("\x1b]8;;\x1b\\");
        }
        if self.style != Style::PLAIN {
            out.push_str("\x1b[0m");
            self.style = Style::PLAIN;
        }
    }
}

fn sgr(style: &Style, out: &mut String) {
    out.push_str("\x1b[0");
    let f = style.flags;
    for (flag, code) in [
        (Flags::BOLD, "1"),
        (Flags::DIM, "2"),
        (Flags::ITALIC, "3"),
        (Flags::UNDERLINE, "4"),
        (Flags::DOUBLE_UNDERLINE, "4:2"),
        (Flags::UNDERCURL, "4:3"),
        (Flags::DOTTED_UNDERLINE, "4:4"),
        (Flags::DASHED_UNDERLINE, "4:5"),
        (Flags::INVERSE, "7"),
        (Flags::HIDDEN, "8"),
        (Flags::STRIKEOUT, "9"),
    ] {
        if f.contains(flag) {
            out.push(';');
            out.push_str(code);
        }
    }
    color(style.fg, 30, 90, "38", out);
    color(style.bg, 40, 100, "48", out);
    if let Some(c) = style.underline {
        match c {
            Color::Spec(rgb) => {
                let _ = write!(out, ";58;2;{};{};{}", rgb.r, rgb.g, rgb.b);
            }
            Color::Indexed(i) => {
                let _ = write!(out, ";58;5;{i}");
            }
            Color::Named(n) => {
                if let Some(i) = ansi_index(n) {
                    let _ = write!(out, ";58;5;{i}");
                }
            }
        }
    }
    out.push('m');
}

fn ansi_index(n: NamedColor) -> Option<u8> {
    let v = n as usize;
    let dim = NamedColor::DimBlack as usize;
    match v {
        0..=15 => Some(v as u8),
        _ if (dim..dim + 8).contains(&v) => Some((v - dim) as u8),
        _ => None,
    }
}

fn color(c: Color, base: u8, bright: u8, extended: &str, out: &mut String) {
    match c {
        Color::Named(n) => match ansi_index(n) {
            Some(i @ 0..=7) => {
                let _ = write!(out, ";{}", base + i);
            }
            Some(i) => {
                let _ = write!(out, ";{}", bright + i - 8);
            }
            None => {}
        },
        Color::Indexed(i) => {
            let _ = write!(out, ";{extended};5;{i}");
        }
        Color::Spec(rgb) => {
            let _ = write!(out, ";{extended};2;{};{};{}", rgb.r, rgb.g, rgb.b);
        }
    }
}

fn private(out: &mut String, code: u16, on: bool) {
    let _ = write!(out, "\x1b[?{code}{}", if on { 'h' } else { 'l' });
}

fn modes(mode: TermMode, out: &mut String) {
    for (flag, code) in [
        (TermMode::APP_CURSOR, 1),
        (TermMode::MOUSE_REPORT_CLICK, 1000),
        (TermMode::MOUSE_DRAG, 1002),
        (TermMode::MOUSE_MOTION, 1003),
        (TermMode::FOCUS_IN_OUT, 1004),
        (TermMode::UTF8_MOUSE, 1005),
        (TermMode::SGR_MOUSE, 1006),
        (TermMode::BRACKETED_PASTE, 2004),
    ] {
        if mode.contains(flag) {
            private(out, code, true);
        }
    }
    for (flag, code) in [
        (TermMode::SHOW_CURSOR, 25),
        (TermMode::LINE_WRAP, 7),
        (TermMode::ALTERNATE_SCROLL, 1007),
        (TermMode::URGENCY_HINTS, 1042),
    ] {
        if !mode.contains(flag) {
            private(out, code, false);
        }
    }
    if mode.contains(TermMode::APP_KEYPAD) {
        out.push_str("\x1b=");
    }
    if mode.contains(TermMode::INSERT) {
        out.push_str("\x1b[4h");
    }
    if mode.contains(TermMode::LINE_FEED_NEW_LINE) {
        out.push_str("\x1b[20h");
    }
    let mut keyboard = 0u8;
    for (bit, flag) in [
        (1, TermMode::DISAMBIGUATE_ESC_CODES),
        (2, TermMode::REPORT_EVENT_TYPES),
        (4, TermMode::REPORT_ALTERNATE_KEYS),
        (8, TermMode::REPORT_ALL_KEYS_AS_ESC),
        (16, TermMode::REPORT_ASSOCIATED_TEXT),
    ] {
        if mode.contains(flag) {
            keyboard |= bit;
        }
    }
    if keyboard != 0 {
        let _ = write!(out, "\x1b[>{keyboard}u");
    }
}

/// A block is the daemon's default, so it is left out: the app's own default
/// (which may be a beam or an underline) then shows until the program asks again.
fn cursor_shape<T>(term: &Term<T>, out: &mut String) {
    let style = term.cursor_style();
    let steady = match style.shape {
        CursorShape::Block => return,
        CursorShape::Underline => 4,
        CursorShape::Beam => 6,
        CursorShape::HollowBlock | CursorShape::Hidden => return,
    };
    let _ = write!(out, "\x1b[{} q", steady - u8::from(style.blinking));
}

fn content_end(row: &alacritty_terminal::grid::Row<Cell>, cols: usize) -> usize {
    (0..cols).rev().find(|&c| !row[Column(c)].is_empty()).map_or(0, |c| c + 1)
}

pub(crate) fn replay<T>(term: &Term<T>) -> Vec<u8> {
    let mut out = String::new();
    let mode = *term.mode();
    if mode.contains(TermMode::ALT_SCREEN) {
        private(&mut out, 1049, true);
    }
    let grid = term.grid();
    let cols = grid.columns();
    let top = grid.topmost_line().0;
    let bottom = grid.bottommost_line().0;
    let mut pen = Pen::new();
    for line in top..=bottom {
        let row = &grid[Line(line)];
        let wrapped = row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
        let end = if wrapped { cols } else { content_end(row, cols) };
        for col in 0..end {
            let cell = &row[Column(col)];
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            pen.take(cell, &mut out);
            out.push(if cell.c == '\0' { ' ' } else { cell.c });
            if let Some(extra) = cell.zerowidth() {
                out.extend(extra);
            }
        }
        if line < bottom && !wrapped {
            pen.reset(&mut out);
            out.push_str("\r\n");
        }
    }
    pen.reset(&mut out);
    let cursor = &grid.cursor;
    let _ = write!(out, "\x1b[{};{}H", cursor.point.line.0 + 1, cursor.point.column.0 + 1);
    let template = Style::of(&cursor.template);
    if template != Style::PLAIN {
        sgr(&template, &mut out);
    }
    modes(mode, &mut out);
    cursor_shape(term, &mut out);
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::event::VoidListener;
    use alacritty_terminal::term::Config;
    use alacritty_terminal::vte::ansi::Processor;

    use super::*;
    use crate::session::GridSize;

    const SIZE: GridSize = GridSize { cols: 20, rows: 5, cell_width: 8, cell_height: 16 };

    fn term_with(bytes: &[u8]) -> Term<VoidListener> {
        let mut term = Term::new(Config { scrolling_history: 100, ..Config::default() }, &SIZE, VoidListener);
        Processor::<alacritty_terminal::vte::ansi::StdSyncHandler>::new().advance(&mut term, bytes);
        term
    }

    fn dump(term: &Term<VoidListener>) -> Vec<String> {
        let grid = term.grid();
        (grid.topmost_line().0..=grid.bottommost_line().0)
            .map(|l| {
                let row = &grid[Line(l)];
                (0..grid.columns())
                    .map(|c| {
                        let cell = &row[Column(c)];
                        format!("{}{:?}{:?}{:?}", cell.c, cell.fg, cell.bg, cell.flags & !Flags::WRAPLINE)
                    })
                    .collect::<Vec<_>>()
                    .join("|")
                    + if row[Column(grid.columns() - 1)].flags.contains(Flags::WRAPLINE) { "↩" } else { "" }
            })
            .collect()
    }

    fn round_trip(bytes: &[u8]) -> (Term<VoidListener>, Term<VoidListener>) {
        let original = term_with(bytes);
        let copy = term_with(&replay(&original));
        (original, copy)
    }

    fn assert_same(bytes: &[u8]) {
        let (a, b) = round_trip(bytes);
        assert_eq!(dump(&a), dump(&b));
        assert_eq!(a.grid().cursor.point, b.grid().cursor.point);
        assert_eq!(*a.mode(), *b.mode());
    }

    #[test]
    fn only_a_cursor_shape_the_program_chose_is_replayed() {
        let shape = |bytes: &[u8]| {
            let mut out = String::new();
            cursor_shape(&term_with(bytes), &mut out);
            out
        };
        assert_eq!(shape(b""), "");
        assert_eq!(shape(b"\x1b[1 q"), "");
        assert_eq!(shape(b"\x1b[6 q"), "\x1b[6 q");
        assert_eq!(shape(b"\x1b[3 q"), "\x1b[3 q");
    }

    #[test]
    fn plain_lines_and_cursor() {
        assert_same(b"oi\r\nmundo\r\n  x");
    }

    #[test]
    fn scrollback_survives() {
        let text: String = (0..30).map(|i| format!("linha {i}\r\n")).collect();
        let (a, b) = round_trip(text.as_bytes());
        assert_eq!(a.grid().history_size(), b.grid().history_size());
        assert_eq!(dump(&a), dump(&b));
    }

    #[test]
    fn colors_and_attributes() {
        assert_same(b"\x1b[1;31mvermelho\x1b[0m \x1b[38;5;200;48;2;1;2;3mcor\x1b[0m\r\n\x1b[3;4;7mfim\x1b[0m");
    }

    #[test]
    fn colored_background_to_the_end_of_line() {
        assert_same(b"\x1b[44m    \x1b[0m\r\nok");
    }

    #[test]
    fn wrapped_lines_stay_wrapped() {
        assert_same(b"0123456789abcdefghijKLMNOP\r\nnext");
    }

    #[test]
    fn wide_chars() {
        assert_same("ab日本語\r\n0123456789012345678日x".as_bytes());
    }

    #[test]
    fn modes_are_restored() {
        assert_same(b"\x1b[?2004h\x1b[?1h\x1b[?1000h\x1b[?1006h\x1b[?25l\x1b[>1uhi");
    }

    #[test]
    fn alt_screen_is_restored() {
        assert_same(b"primary\r\n\x1b[?1049h\x1b[2;3Halt");
    }

    #[test]
    fn hyperlinks_survive() {
        let (_, b) = round_trip(b"\x1b]8;;https://x.dev\x1b\\link\x1b]8;;\x1b\\ fim");
        let cell = &b.grid()[Line(0)][Column(1)];
        assert_eq!(cell.hyperlink().map(|h| h.uri().to_owned()).as_deref(), Some("https://x.dev"));
        assert!(b.grid()[Line(0)][Column(5)].hyperlink().is_none());
    }
}
