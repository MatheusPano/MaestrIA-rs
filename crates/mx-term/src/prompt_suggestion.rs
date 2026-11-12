use alacritty_terminal::grid::{Dimensions, Row};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::{Cell, Flags};

const PROMPT: char = '❯';
const RULE: char = '─';
const TIP: &str = "Try \"";
const SPACER: Flags = Flags::WIDE_CHAR_SPACER.union(Flags::LEADING_WIDE_CHAR_SPACER);

fn blank(c: char) -> bool {
    c == ' ' || c == '\0' || c == '\u{a0}'
}

/// The placeholder Claude Code draws in an empty prompt (dim, first cell inverse when focused): its suggested next prompt.
pub(crate) fn prompt_suggestion<T>(term: &Term<T>) -> Option<String> {
    let grid = term.grid();
    let lines = term.screen_lines() as i32;
    let (top, at) = (0..lines).rev().find_map(|l| prompt_at(&grid[Line(l)]).map(|at| (l, at)))?;
    let mut text = dim_text(&grid[Line(top)], at + 1, true)?;
    for l in top + 1..lines {
        let Some(more) = dim_text(&grid[Line(l)], 0, false).filter(|m| !m.trim().is_empty() && !m.contains(RULE)) else {
            break;
        };
        let above = &grid[Line(l - 1)];
        if above[Column(above.len() - 1)].flags.contains(Flags::WRAPLINE) {
            text.push_str(&more);
        } else {
            text = format!("{} {}", text.trim_end(), more.trim_start());
        }
    }
    let text = text.trim().to_owned();
    (!text.is_empty() && !text.starts_with(TIP)).then_some(text)
}

fn prompt_at(row: &Row<Cell>) -> Option<usize> {
    let at = (0..row.len().min(3)).find(|&c| !blank(row[Column(c)].c))?;
    (row[Column(at)].c == PROMPT).then_some(at)
}

fn dim_text(row: &Row<Cell>, from: usize, cursor_first: bool) -> Option<String> {
    let mut out = String::new();
    let mut seen = false;
    for c in from..row.len() {
        let cell = &row[Column(c)];
        if cell.flags.intersects(SPACER) {
            continue;
        }
        if !blank(cell.c) {
            let cursor = cursor_first && !seen && cell.flags.contains(Flags::INVERSE);
            if !cursor && !cell.flags.contains(Flags::DIM) {
                return None;
            }
            seen = true;
        }
        out.push(if blank(cell.c) { ' ' } else { cell.c });
    }
    Some(out)
}
