use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
use mx_theme::{Rgb, TerminalPalette};

const DIM_FACTOR: f32 = 0.66;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellStyle {
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapCell {
    pub c: char,
    pub style: CellStyle,
    pub wide: bool,
    pub spacer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorKind {
    Block,
    Underline,
    Beam,
    HollowBlock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapCursor {
    pub row: usize,
    pub col: usize,
    pub kind: CursorKind,
    pub color: Rgb,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<SnapCell>,
    pub cursor: Option<SnapCursor>,
    pub foreground: Rgb,
    pub background: Rgb,
    pub mode: TermMode,
    pub display_offset: usize,
}

impl Snapshot {
    pub fn row(&self, row: usize) -> &[SnapCell] {
        &self.cells[row * self.cols..(row + 1) * self.cols]
    }
}

pub(crate) fn snapshot<T: EventListener>(term: &Term<T>, palette: &TerminalPalette) -> Snapshot {
    let content = term.renderable_content();
    let cols = term.columns();
    let rows = term.screen_lines();
    let colors = content.colors;
    let foreground = named(NamedColor::Foreground, colors, palette);
    let background = named(NamedColor::Background, colors, palette);
    let blank = SnapCell {
        c: ' ',
        style: CellStyle {
            fg: foreground,
            bg: background,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
        },
        wide: false,
        spacer: false,
    };
    let mut cells = vec![blank; cols * rows];
    let offset = content.display_offset as i32;

    for indexed in content.display_iter {
        let row = indexed.point.line.0 + offset;
        let col = indexed.point.column.0;
        if row < 0 || row as usize >= rows || col >= cols {
            continue;
        }
        let cell = indexed.cell;
        let flags = cell.flags;
        let fg_color = match (cell.fg, flags.contains(Flags::DIM)) {
            (Color::Named(n), true) => Color::Named(n.to_dim()),
            (other, _) => other,
        };
        let mut fg = resolve(fg_color, colors, palette);
        let mut bg = resolve(cell.bg, colors, palette);
        if flags.contains(Flags::DIM) && !matches!(fg_color, Color::Named(_)) {
            fg = fg.scaled(DIM_FACTOR);
        }
        if flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if flags.contains(Flags::HIDDEN) {
            fg = bg;
        }
        cells[row as usize * cols + col] = SnapCell {
            c: cell.c,
            style: CellStyle {
                fg,
                bg,
                bold: flags.contains(Flags::BOLD),
                italic: flags.contains(Flags::ITALIC),
                underline: flags.intersects(Flags::ALL_UNDERLINES),
                strikeout: flags.contains(Flags::STRIKEOUT),
            },
            wide: flags.contains(Flags::WIDE_CHAR),
            spacer: flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER),
        };
    }

    let cursor_row = content.cursor.point.line.0 + offset;
    let kind = match content.cursor.shape {
        CursorShape::Block => Some(CursorKind::Block),
        CursorShape::Underline => Some(CursorKind::Underline),
        CursorShape::Beam => Some(CursorKind::Beam),
        CursorShape::HollowBlock => Some(CursorKind::HollowBlock),
        CursorShape::Hidden => None,
    };
    let cursor = kind
        .filter(|_| cursor_row >= 0 && (cursor_row as usize) < rows)
        .map(|kind| SnapCursor {
            row: cursor_row as usize,
            col: content.cursor.point.column.0.min(cols.saturating_sub(1)),
            kind,
            color: named(NamedColor::Cursor, colors, palette),
        });

    Snapshot {
        cols,
        rows,
        cells,
        cursor,
        foreground,
        background,
        mode: content.mode,
        display_offset: content.display_offset,
    }
}

fn resolve(color: Color, colors: &Colors, palette: &TerminalPalette) -> Rgb {
    match color {
        Color::Spec(c) => Rgb {
            r: c.r,
            g: c.g,
            b: c.b,
        },
        Color::Indexed(i) => colors[usize::from(i)]
            .map(|c| Rgb {
                r: c.r,
                g: c.g,
                b: c.b,
            })
            .unwrap_or_else(|| palette.indexed(i)),
        Color::Named(n) => named(n, colors, palette),
    }
}

fn named(n: NamedColor, colors: &Colors, palette: &TerminalPalette) -> Rgb {
    if let Some(c) = colors[n] {
        return Rgb {
            r: c.r,
            g: c.g,
            b: c.b,
        };
    }
    let index = n as usize;
    match n {
        _ if index < 16 => palette.ansi[index],
        NamedColor::Foreground | NamedColor::BrightForeground => palette.foreground,
        NamedColor::Background => palette.background,
        NamedColor::Cursor => palette.cursor,
        NamedColor::DimForeground => palette.foreground.scaled(DIM_FACTOR),
        _ => {
            let base = index - NamedColor::DimBlack as usize;
            palette.ansi[base.min(7)].scaled(DIM_FACTOR)
        }
    }
}
