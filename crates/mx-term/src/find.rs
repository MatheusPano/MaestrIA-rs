use regex::RegexBuilder;

use crate::rows::{CellPos, Row};

pub const FIND_LIMIT: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hit {
    pub from: CellPos,
    pub to: CellPos,
}

impl Hit {
    pub fn new(from: (usize, usize), to: (usize, usize)) -> Self {
        Self { from: CellPos::new(from.0, from.1), to: CellPos::new(to.0, to.1) }
    }
}

pub fn find_in_rows(rows: &[Row], width: usize, query: &str, limit: usize) -> Vec<Hit> {
    find_in(rows.iter().cloned(), width, query, limit)
}

pub(crate) fn find_in(rows: impl IntoIterator<Item = Row>, width: usize, query: &str, limit: usize) -> Vec<Hit> {
    if query.is_empty() {
        return Vec::new();
    }
    let sensitive = query != query.to_lowercase();
    let Ok(pattern) = RegexBuilder::new(&regex::escape(query)).case_insensitive(!sensitive).build() else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    let mut group: Vec<Row> = Vec::new();
    let mut first = 0;
    let mut rows = rows.into_iter().peekable();
    while let Some(row) = rows.next() {
        group.push(row);
        if rows.peek().is_some_and(|next| next.wrapped) {
            continue;
        }
        scan(&group, first, width, &pattern, limit, &mut hits);
        if hits.len() >= limit {
            break;
        }
        first += group.len();
        group.clear();
    }
    hits
}

fn scan(group: &[Row], first: usize, width: usize, pattern: &regex::Regex, limit: usize, hits: &mut Vec<Hit>) {
    let mut text = String::new();
    let mut cells: Vec<(usize, usize, usize)> = Vec::new();
    for (i, line) in group.iter().enumerate() {
        for x in 0..width.min(line.len()) {
            let cell = &line.cells[x];
            if cell.c == '\0' && x > 0 && line.width(x - 1) == 2 {
                continue;
            }
            cells.push((text.len(), i, x));
            if cell.c == '\0' {
                text.push(' ');
            } else {
                cell.push_to(&mut text);
            }
        }
    }
    let at = |byte: usize| {
        let (_, i, x) = cells[cells.partition_point(|(b, _, _)| *b <= byte) - 1];
        (i, x)
    };
    for m in pattern.find_iter(&text) {
        let (fi, fx) = at(m.start());
        let (ei, ex) = at(m.end().saturating_sub(1).max(m.start()));
        let span = if group[ei].width(ex) == 2 { 2 } else { 1 };
        hits.push(Hit { from: CellPos::new(fx, first + fi), to: CellPos::new(ex + span, first + ei) });
        if hits.len() >= limit {
            break;
        }
    }
}
