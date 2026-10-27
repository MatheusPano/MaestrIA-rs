use std::sync::atomic::Ordering;

use alacritty_terminal::grid::{Dimensions, Scroll};

use crate::find::{FIND_LIMIT, Hit, find_in};
use crate::rows::{self, CellPos};
use crate::session::TerminalSession;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub history: usize,
    pub display_offset: usize,
    pub rows: usize,
}

impl Viewport {
    pub fn total(&self) -> usize {
        self.history + self.rows
    }

    pub fn top(&self) -> usize {
        self.history.saturating_sub(self.display_offset)
    }

    pub fn bottom(&self) -> usize {
        self.top() + self.rows.saturating_sub(1)
    }

    pub fn row_of(&self, y: usize) -> Option<usize> {
        y.checked_sub(self.top()).filter(|r| *r < self.rows)
    }

    pub fn shows(&self, hit: &Hit) -> bool {
        hit.from.y >= self.top() && hit.to.y <= self.bottom()
    }

    pub fn centring(&self, y: usize) -> usize {
        let max = self.total().saturating_sub(self.rows);
        y.saturating_sub(self.rows.saturating_sub(1) / 2).min(max)
    }
}

impl TerminalSession {
    pub fn find(&self, query: &str, limit: usize) -> Vec<Hit> {
        if query.is_empty() {
            return Vec::new();
        }
        let term = self.shared.term.lock();
        let width = term.columns();
        find_in((0..rows::total(&*term)).map(|y| rows::read_row(&*term, y)), width, query, limit)
    }

    pub fn viewport(&self) -> Viewport {
        let term = self.shared.term.lock();
        Viewport {
            history: rows::history(&*term),
            display_offset: term.grid().display_offset(),
            rows: term.screen_lines(),
        }
    }

    pub fn scroll_to_top(&self, top: usize) {
        let mut term = self.shared.term.lock();
        let offset = rows::history(&*term).saturating_sub(top) as i32;
        let delta = offset - term.grid().display_offset() as i32;
        if delta != 0 {
            term.scroll_display(Scroll::Delta(delta));
        }
        self.shared.scrolled.store(term.grid().display_offset() != 0, Ordering::Release);
        drop(term);
        self.shared.port.wake();
    }

    pub(crate) fn line_text(&self, y: usize) -> Option<String> {
        let term = self.shared.term.lock();
        (y < rows::total(&*term)).then(|| rows::read_row(&*term, y).text())
    }

    fn line_at_or_above(&self, y: usize, text: &str) -> Option<usize> {
        let term = self.shared.term.lock();
        let last = y.min(rows::total(&*term).checked_sub(1)?);
        (0..=last).rev().find(|&at| rows::read_row(&*term, at).text() == text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Anchor {
    at: CellPos,
    text: String,
}

#[derive(Debug, Default)]
pub struct FindSession {
    query: String,
    hits: Vec<Hit>,
    index: Option<usize>,
    anchor: Option<Anchor>,
}

impl FindSession {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    pub fn index(&self) -> Option<usize> {
        self.index
    }

    pub fn current(&self) -> Option<Hit> {
        self.index.and_then(|i| self.hits.get(i).copied())
    }

    pub fn counter(&self) -> Option<String> {
        if self.query.is_empty() {
            return None;
        }
        let count = self.hits.len();
        Some(match self.index {
            _ if count == 0 => "Nenhum".to_owned(),
            Some(i) => format!("{} de {count}{}", i + 1, if count >= FIND_LIMIT { "+" } else { "" }),
            None => format!("0 de {count}"),
        })
    }

    pub fn search(&mut self, term: &TerminalSession, query: &str, near_line: usize) {
        self.query = query.to_owned();
        self.hits = term.find(query, FIND_LIMIT);
        self.index = self.hits.iter().rposition(|h| h.from.y <= near_line).or((!self.hits.is_empty()).then_some(0));
        self.mark(term);
    }

    pub fn refresh(&mut self, term: &TerminalSession) {
        if self.query.is_empty() {
            return;
        }
        let was = self.anchor.take().and_then(|a| locate(term, &a));
        self.hits = term.find(&self.query, FIND_LIMIT);
        self.index = match (self.hits.len(), was) {
            (0, _) => None,
            (n, None) => Some(n - 1),
            (n, Some(was)) => Some(self.hits.iter().position(|h| h.from >= was).unwrap_or(n - 1)),
        };
        self.mark(term);
    }

    pub fn step(&mut self, term: &TerminalSession, by: i32) {
        let len = self.hits.len() as i64;
        if len == 0 {
            return;
        }
        let at = self.index.map_or(-1, |i| i as i64);
        self.index = Some((at + by as i64).rem_euclid(len) as usize);
        self.mark(term);
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.hits.clear();
        self.index = None;
        self.anchor = None;
    }

    fn mark(&mut self, term: &TerminalSession) {
        self.anchor = self.current().and_then(|h| term.line_text(h.from.y).map(|text| Anchor { at: h.from, text }));
    }
}

fn locate(term: &TerminalSession, anchor: &Anchor) -> Option<CellPos> {
    term.line_at_or_above(anchor.at.y, &anchor.text).map(|y| CellPos::new(anchor.at.x, y))
}
