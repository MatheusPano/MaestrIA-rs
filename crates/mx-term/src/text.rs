use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Flags;

use crate::links::{link_at_chars, local_path};

pub(crate) fn viewport_point<T>(term: &Term<T>, col: usize, row: usize) -> Option<Point> {
    let rows = term.screen_lines();
    let cols = term.columns();
    if row >= rows || col >= cols {
        return None;
    }
    let offset = term.grid().display_offset() as i32;
    Some(Point::new(Line(row as i32 - offset), Column(col)))
}

fn wraps<T>(term: &Term<T>, line: Line) -> bool {
    let last = Column(term.columns() - 1);
    term.grid()[line][last].flags.contains(Flags::WRAPLINE)
}

fn push_row<T>(term: &Term<T>, line: Line, out: &mut Vec<char>) {
    let row = &term.grid()[line];
    for col in 0..term.columns() {
        let cell = &row[Column(col)];
        let spacer = cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER);
        out.push(if spacer || cell.c == '\0' { ' ' } else { cell.c });
    }
}

pub(crate) fn logical_line<T>(term: &Term<T>, point: Point) -> (Vec<char>, usize) {
    let top = term.grid().topmost_line();
    let bottom = term.grid().bottommost_line();
    let mut first = point.line;
    while first > top && wraps(term, first - 1) {
        first -= 1;
    }
    let mut last = point.line;
    while last < bottom && wraps(term, last) {
        last += 1;
    }
    let mut text = Vec::with_capacity(term.columns() * (1 + (last.0 - first.0) as usize));
    let mut line = first;
    while line <= last {
        push_row(term, line, &mut text);
        line += 1;
    }
    let column = (point.line.0 - first.0) as usize * term.columns() + point.column.0;
    (text, column)
}

pub(crate) fn hyperlink_at<T>(term: &Term<T>, point: Point) -> Option<String> {
    let cell = &term.grid()[point];
    let cell = if cell.flags.contains(Flags::WIDE_CHAR_SPACER) && point.column.0 > 0 {
        &term.grid()[Point::new(point.line, point.column - 1)]
    } else {
        cell
    };
    cell.hyperlink().map(|h| local_path(h.uri()))
}

pub(crate) fn link_at_cell<T>(term: &Term<T>, col: usize, row: usize, base: Option<&str>) -> Option<String> {
    let point = viewport_point(term, col, row)?;
    if let Some(link) = hyperlink_at(term, point) {
        return Some(link);
    }
    let (line, column) = logical_line(term, point);
    link_at_chars(&line, column, base)
}

pub(crate) fn row_text<T>(term: &Term<T>, row: usize) -> String {
    let Some(point) = viewport_point(term, 0, row) else {
        return String::new();
    };
    let mut out = Vec::new();
    push_row(term, point.line, &mut out);
    out.into_iter().collect::<String>().trim_end().to_owned()
}
