use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

use alacritty_terminal::index::{Column, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::{Term, TermMode};

use crate::io::{Port, Proxy, SharedTerm};
use crate::rows::{self, CellPos};
use crate::session::lock;

pub(crate) const SETTLE: Duration = Duration::from_millis(30);
pub(crate) const GRACE: Duration = Duration::from_millis(120);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Saved {
    begin: CellPos,
    end: CellPos,
    alt: bool,
    history: usize,
    rows: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Sticky {
    saved: Option<Saved>,
    deadline: Option<Instant>,
    missed: bool,
}

fn alt<T>(term: &Term<T>) -> bool {
    term.mode().contains(TermMode::ALT_SCREEN)
}

fn current<T>(term: &Term<T>) -> Option<(CellPos, CellPos)> {
    let range = term.selection.as_ref().filter(|s| !s.is_empty())?.to_range(term)?;
    let at = |p: Point| CellPos::new(p.column.0, rows::y_of(term, p.line));
    Some((at(range.start), at(range.end)))
}

fn row_texts<T>(term: &Term<T>, top: usize, bottom: usize) -> Vec<String> {
    rows::read_rows(term, top, bottom + 1).iter().map(|r| r.text()).collect()
}

fn select<T>(term: &mut Term<T>, begin: CellPos, end: CellPos) {
    let point = |p: CellPos| Point::new(rows::line_of(term, p.y), Column(p.x));
    let mut sel = Selection::new(SelectionType::Simple, point(begin), Side::Left);
    sel.update(point(end), Side::Right);
    term.selection = Some(sel);
}

impl Sticky {
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub fn selected<T>(&mut self, term: &Term<T>) {
        let Some((begin, end)) = current(term) else { return self.forget() };
        let alt = alt(term);
        if self.saved.as_ref().is_some_and(|s| s.begin == begin && s.end == end && s.alt == alt) {
            return;
        }
        let rows = if alt { row_texts(term, begin.y, end.y) } else { Vec::new() };
        self.saved = Some(Saved { begin, end, alt, history: rows::history(term), rows });
        self.missed = false;
    }

    pub fn written(&mut self, now: Instant) -> bool {
        if self.saved.is_none() || self.deadline.is_some() {
            return false;
        }
        self.deadline = Some(now + SETTLE);
        true
    }

    pub fn fire<T>(&mut self, term: &mut Term<T>, now: Instant) -> bool {
        match self.deadline {
            Some(d) if d <= now => {
                self.deadline = None;
                self.follow(term, now)
            }
            _ => false,
        }
    }

    fn follow<T>(&mut self, term: &mut Term<T>, now: Instant) -> bool {
        let Some(saved) = self.saved.clone() else { return false };
        if saved.alt != alt(term) {
            self.forget();
            return false;
        }
        if !saved.alt {
            return self.keep(term, saved);
        }
        let live = current(term);
        let (begin, end) = live.unwrap_or((saved.begin, saved.end));
        let screen = row_texts(term, 0, rows::total(term).saturating_sub(1));
        let Some(shift) = find_shift(&saved.rows, begin.y, &screen) else {
            if !self.missed {
                self.missed = true;
                self.deadline = Some(now + GRACE);
                return false;
            }
            term.selection = None;
            self.forget();
            return true;
        };
        self.missed = false;
        if shift == 0 && live.is_some() {
            self.resave(begin, end);
            return false;
        }
        let moved = |p: CellPos| CellPos::new(p.x, (p.y as i64 + shift) as usize);
        let (begin, end) = (moved(begin), moved(end));
        select(term, begin, end);
        self.resave(begin, end);
        true
    }

    fn keep<T>(&mut self, term: &mut Term<T>, saved: Saved) -> bool {
        if let Some((begin, end)) = current(term) {
            self.resave(begin, end);
            return false;
        }
        if rows::history(term) < saved.history || saved.end.y >= rows::total(term) {
            self.forget();
            return false;
        }
        select(term, saved.begin, saved.end);
        true
    }

    fn resave(&mut self, begin: CellPos, end: CellPos) {
        if let Some(s) = self.saved.as_mut() {
            s.begin = begin;
            s.end = end;
        }
    }

    fn forget(&mut self) {
        self.saved = None;
        self.deadline = None;
        self.missed = false;
    }
}

pub(crate) fn find_shift(rows: &[String], top: usize, screen: &[String]) -> Option<i64> {
    let height = screen.len();
    let matches = |at: usize| rows.iter().zip(&screen[at..]).all(|(a, b)| a == b);
    for distance in 0..height as i64 {
        let shifts: &[i64] = if distance == 0 { &[0] } else { &[distance, -distance] };
        for &shift in shifts {
            let at = top as i64 + shift;
            if at < 0 || at as usize + rows.len() > height {
                continue;
            }
            if matches(at as usize) {
                return Some(shift);
            }
        }
    }
    None
}

#[derive(Default)]
struct Clock {
    sticky: Sticky,
    running: bool,
    term: Option<Weak<alacritty_terminal::sync::FairMutex<Term<Proxy>>>>,
    port: Option<Weak<Port>>,
}

#[derive(Default)]
pub(crate) struct StickyClock {
    state: Mutex<Clock>,
    cv: Condvar,
}

impl StickyClock {
    pub fn attach(&self, term: &SharedTerm, port: &Arc<Port>) {
        let mut st = lock(&self.state);
        st.term = Some(Arc::downgrade(term));
        st.port = Some(Arc::downgrade(port));
    }

    pub fn selected(&self, term: &Term<Proxy>) {
        lock(&self.state).sticky.selected(term);
    }

    pub fn written(self: &Arc<Self>) {
        let mut st = lock(&self.state);
        if !st.sticky.written(Instant::now()) {
            return;
        }
        if st.running {
            self.cv.notify_all();
            return;
        }
        st.running = true;
        drop(st);
        let me = self.clone();
        let spawned = std::thread::Builder::new().name("mx-sticky".into()).spawn(move || me.run());
        if spawned.is_err() {
            lock(&self.state).running = false;
        }
    }

    fn run(&self) {
        loop {
            let mut st = lock(&self.state);
            let (Some(term), Some(port)) =
                (st.term.as_ref().and_then(Weak::upgrade), st.port.as_ref().and_then(Weak::upgrade))
            else {
                st.running = false;
                return;
            };
            loop {
                let now = Instant::now();
                match st.sticky.deadline() {
                    None => {
                        st.running = false;
                        return;
                    }
                    Some(d) if d > now => {
                        st = self.cv.wait_timeout(st, d - now).map(|(g, _)| g).unwrap_or_else(|e| e.into_inner().0);
                    }
                    Some(_) => break,
                }
            }
            drop(st);
            let changed = {
                let mut t = term.lock();
                lock(&self.state).sticky.fire(&mut t, Instant::now())
            };
            if changed {
                port.wake();
            }
        }
    }
}

#[cfg(test)]
#[path = "sticky_tests.rs"]
mod tests;
