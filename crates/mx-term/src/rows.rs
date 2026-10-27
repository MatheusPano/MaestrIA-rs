use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Flags;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowCell {
    pub c: char,
    pub width: u8,
    pub marks: Option<Box<[char]>>,
}

impl RowCell {
    pub fn new(c: char, width: u8) -> Self {
        Self { c, width, marks: None }
    }

    pub fn blank(&self) -> bool {
        self.c == '\0' || self.c == ' '
    }

    pub(crate) fn push_to(&self, out: &mut String) {
        out.push(self.c);
        if let Some(marks) = &self.marks {
            out.extend(marks.iter());
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<RowCell>,
    pub wrapped: bool,
}

impl Row {
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn code(&self, i: usize) -> char {
        self.cells.get(i).map_or('\0', |c| c.c)
    }

    pub fn blank(&self, i: usize) -> bool {
        self.cells.get(i).is_none_or(RowCell::blank)
    }

    pub fn width(&self, i: usize) -> usize {
        self.cells.get(i).map_or(1, |c| usize::from(c.width))
    }

    pub fn text(&self) -> String {
        let mut out = String::with_capacity(self.cells.len());
        for cell in &self.cells {
            if cell.c == '\0' {
                out.push(' ');
            } else {
                cell.push_to(&mut out);
            }
        }
        out.truncate(out.trim_end().len());
        out
    }
}

pub(crate) fn history<T>(term: &Term<T>) -> usize {
    term.grid().history_size()
}

pub(crate) fn total<T>(term: &Term<T>) -> usize {
    history(term) + term.screen_lines()
}

pub(crate) fn line_of<T>(term: &Term<T>, y: usize) -> Line {
    Line(y as i32 - history(term) as i32)
}

pub(crate) fn y_of<T>(term: &Term<T>, line: Line) -> usize {
    (line.0 + history(term) as i32).max(0) as usize
}

pub(crate) fn read_row<T>(term: &Term<T>, y: usize) -> Row {
    let cols = term.columns();
    let line = line_of(term, y);
    let grid = term.grid();
    let row = &grid[line];
    let mut cells = Vec::with_capacity(cols);
    for col in 0..cols {
        let cell = &row[Column(col)];
        let f = cell.flags;
        let rc = if f.contains(Flags::WIDE_CHAR_SPACER) {
            RowCell::new('\0', 0)
        } else if f.contains(Flags::LEADING_WIDE_CHAR_SPACER) || cell.c == ' ' {
            RowCell::new('\0', 1)
        } else {
            RowCell {
                c: cell.c,
                width: if f.contains(Flags::WIDE_CHAR) { 2 } else { 1 },
                marks: cell.zerowidth().map(|z| z.to_vec().into_boxed_slice()),
            }
        };
        cells.push(rc);
    }
    let wrapped = y > 0 && grid[Point::new(line - 1, Column(cols - 1))].flags.contains(Flags::WRAPLINE);
    Row { cells, wrapped }
}

pub(crate) fn read_rows<T>(term: &Term<T>, from: usize, to: usize) -> Vec<Row> {
    (from..to.min(total(term))).map(|y| read_row(term, y)).collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CellPos {
    pub y: usize,
    pub x: usize,
}

impl CellPos {
    pub fn new(x: usize, y: usize) -> Self {
        Self { y, x }
    }
}

pub(crate) struct Lines<'a> {
    pub rows: &'a [Row],
    pub base: usize,
}

impl Lines<'_> {
    pub fn get(&self, y: usize) -> Option<&Row> {
        y.checked_sub(self.base).and_then(|i| self.rows.get(i))
    }
}
