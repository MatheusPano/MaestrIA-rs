use std::collections::HashSet;
use std::sync::LazyLock;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Term;
use regex::Regex;

use crate::rows::{self, CellPos, Lines, Row};
use crate::session::TerminalSession;

static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([●⏺•⎿❯>*-]|[0-9]+[.)])$").expect("marker regex"));
static REPLY_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[●⏺] +").expect("reply regex"));

pub(crate) fn selected_text(lines: &Lines<'_>, height: usize, width: usize, begin: CellPos, end: CellPos, reflow: bool) -> String {
    let (begin, end) = if (begin.y, begin.x) <= (end.y, end.x) { (begin, end) } else { (end, begin) };
    let mut out: Vec<Vec<String>> = vec![Vec::new()];
    let mut hanging = 0;
    for y in begin.y..=end.y {
        if y >= height {
            continue;
        }
        let Some(line) = lines.get(y) else { continue };
        let start = if y == begin.y { begin.x } else { 0 };
        let stop = if y == end.y { end.x } else { width };
        let text = segment_text(line, start, stop);
        let first = y == begin.y || y == 0;
        if first || line.wrapped {
            push_part(&mut out, text);
            if first {
                hanging = hanging_indent(line);
            }
            continue;
        }
        let above = lines.get(y - 1);
        if reflow && let Some(above) = above.filter(|a| claude_wrapped(a, line, width, hanging)) {
            let space = if broken_word(above, width, hanging) { "" } else { " " };
            if let Some(last) = out.last_mut().and_then(|l| l.last_mut()) {
                last.truncate(last.trim_end().len());
            }
            push_part(&mut out, format!("{space}{}", text.trim_start()));
            continue;
        }
        out.push(vec![text]);
        hanging = hanging_indent(line);
    }
    let text: Vec<String> = out.into_iter().map(|parts| parts.concat()).collect();
    if !reflow {
        return text.join("\n");
    }
    let from_indent = begin.y < height && lines.get(begin.y).is_some_and(|l| begin.x <= indent(l));
    let trimmed = text.iter().map(|l| l.trim_end().to_owned()).collect();
    dedent(trimmed, from_indent).join("\n")
}

fn push_part(out: &mut [Vec<String>], text: String) {
    if let Some(last) = out.last_mut() {
        last.push(text);
    }
}

pub(crate) fn segment_text(line: &Row, start: usize, end: usize) -> String {
    let to = end.min(line.len());
    let from = start.min(to);
    let mut drawn = to;
    while drawn > from && line.code(drawn - 1) == '\0' {
        drawn -= 1;
    }
    let mut out = String::new();
    for i in from..drawn {
        let cell = &line.cells[i];
        if cell.c == '\0' {
            if i > 0 && line.width(i - 1) == 2 {
                continue;
            }
            out.push(' ');
            continue;
        }
        if i + line.width(i) <= to {
            cell.push_to(&mut out);
        }
    }
    out
}

fn claude_wrapped(above: &Row, below: &Row, width: usize, hanging: usize) -> bool {
    let end = drawn_end(above);
    let ind = indent(below);
    if end == 0 || ind >= below.len() || ind != hanging {
        return false;
    }
    if is_box(above.code(end - 1)) || is_box(below.code(ind)) {
        return false;
    }
    end + 1 + word_width(below, ind) > width
}

fn broken_word(line: &Row, width: usize, hanging: usize) -> bool {
    drawn_end(line) >= width && hanging + word_width(line, hanging) >= width
}

fn hanging_indent(line: &Row) -> usize {
    let start = indent(line);
    let mut i = start;
    while i < line.len() && !line.blank(i) {
        i += 1;
    }
    let word: String = (start..i).map(|j| line.code(j)).collect();
    if !MARKER.is_match(&word) {
        return start;
    }
    while i < line.len() && line.blank(i) {
        i += 1;
    }
    i
}

fn indent(line: &Row) -> usize {
    (0..line.len()).find(|&i| !line.blank(i)).unwrap_or(line.len())
}

fn drawn_end(line: &Row) -> usize {
    (0..line.len()).rev().find(|&i| !line.blank(i)).map_or(0, |i| i + line.width(i))
}

fn word_width(line: &Row, from: usize) -> usize {
    let mut i = from;
    while i < line.len() && !line.blank(i) {
        i += line.width(i).clamp(1, 2);
    }
    i - from
}

fn is_box(c: char) -> bool {
    ('\u{2500}'..='\u{257F}').contains(&c)
}

fn dedent(mut lines: Vec<String>, first_counts: bool) -> Vec<String> {
    let mut kept: HashSet<usize> = HashSet::new();
    if !first_counts {
        kept.insert(0);
    }
    for (i, line) in lines.iter_mut().enumerate() {
        if let Some(end) = REPLY_MARK.find(line).map(|m| m.end()) {
            line.replace_range(..end, "");
            kept.insert(i);
        }
    }
    let shared = lines
        .iter()
        .enumerate()
        .filter(|(i, l)| !kept.contains(i) && !l.trim().is_empty())
        .map(|(_, l)| l.len() - l.trim_start().len())
        .min();
    let Some(shared) = shared else { return lines };
    for (i, line) in lines.iter_mut().enumerate() {
        if !kept.contains(&i) {
            let cut = line.bytes().take(shared).take_while(|b| *b == b' ').count();
            line.replace_range(..cut, "");
        }
    }
    lines
}

pub(crate) fn selection_text_of<T>(term: &Term<T>, reflow: bool) -> Option<String> {
    let range = term.selection.as_ref().filter(|s| !s.is_empty())?.to_range(term)?;
    let begin = CellPos::new(range.start.column.0, rows::y_of(term, range.start.line));
    let end = CellPos::new(range.end.column.0 + 1, rows::y_of(term, range.end.line));
    let base = begin.y.saturating_sub(1);
    let lines = rows::read_rows(term, base, end.y + 1);
    let text = selected_text(&Lines { rows: &lines, base }, rows::total(term), term.columns(), begin, end, reflow);
    (!text.is_empty()).then_some(text)
}

impl TerminalSession {
    pub fn selection_text(&self, reflow: bool) -> Option<String> {
        selection_text_of(&*self.shared.term.lock(), reflow)
    }
}
