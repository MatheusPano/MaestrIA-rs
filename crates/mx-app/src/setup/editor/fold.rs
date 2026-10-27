use gpui::{Context, Pixels, Point, px};

use super::TextEditor;
use super::syntax::{Spans, Tone};

pub const FOLD_WIDTH: f32 = 18.;

pub fn chunks(text: &str, spans: &Spans) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut open: Vec<(char, usize)> = Vec::new();
    let mut line = 0;
    let mut skip = spans.iter().filter(|(_, t)| matches!(t, Tone::String | Tone::Comment)).peekable();
    for (i, c) in text.char_indices() {
        while skip.peek().is_some_and(|(r, _)| r.end <= i) {
            skip.next();
        }
        if c == '\n' {
            line += 1;
            continue;
        }
        if skip.peek().is_some_and(|(r, _)| r.start <= i) {
            continue;
        }
        match c {
            '{' | '[' | '(' => open.push((c, line)),
            '}' | ']' | ')' => {
                let want = match c {
                    '}' => '{',
                    ']' => '[',
                    _ => '(',
                };
                let Some(at) = open.iter().rposition(|(o, _)| *o == want) else { continue };
                let (_, start) = open[at];
                open.truncate(at);
                if line > start + 1 {
                    match out.iter_mut().find(|(s, _)| *s == start) {
                        Some(chunk) => chunk.1 = chunk.1.max(line),
                        None => out.push((start, line)),
                    }
                }
            }
            _ => {}
        }
    }
    out.sort_unstable();
    out
}

pub fn effective(chunks: &[(usize, usize)], folded: impl Iterator<Item = usize>) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for start in folded {
        let Some(&(s, e)) = chunks.iter().find(|(s, _)| *s == start) else { continue };
        if out.last().is_some_and(|(_, end)| s < *end) {
            continue;
        }
        out.push((s, e));
    }
    out
}

impl TextEditor {
    fn line_count(&self) -> usize {
        self.buffer.text().matches('\n').count() + 1
    }

    pub(super) fn refold(&mut self) {
        if !self.folded.is_empty() && self.fold_lines != self.line_count() {
            self.folded.clear();
            self.layout = None;
        }
    }

    pub(super) fn reveal_cursor(&mut self) {
        let Some(layout) = &self.layout else { return };
        if self.folded.is_empty() {
            return;
        }
        let text = self.buffer.text();
        let line = text[..self.buffer.head()].matches('\n').count();
        let hit: Vec<usize> = layout.folded.iter().filter(|(s, e)| *s < line && line < *e).map(|(s, _)| *s).collect();
        if !hit.is_empty() {
            for s in hit {
                self.folded.remove(&s);
            }
            self.layout = None;
        }
    }

    pub fn toggle_fold(&mut self, line: usize, cx: &mut Context<Self>) -> bool {
        let Some(layout) = &self.layout else { return false };
        if !layout.chunks.iter().any(|(s, _)| *s == line) {
            return false;
        }
        if !self.folded.remove(&line) {
            self.folded.insert(line);
            self.fold_lines = self.line_count();
        }
        self.layout = None;
        cx.notify();
        true
    }

    #[cfg(test)]
    pub fn folded_lines(&self) -> Vec<usize> {
        self.folded.iter().copied().collect()
    }

    pub(super) fn fold_click(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        if self.style.gutter.as_ref().is_none_or(|g| g.fold.is_none()) {
            return false;
        }
        let Some(layout) = &self.layout else { return false };
        let x = at.x - self.bounds.origin.x;
        if x < layout.numbers || x > layout.numbers + px(FOLD_WIDTH) {
            return false;
        }
        let y = at.y - self.bounds.origin.y - px(self.style.pad.top) + self.scroll_y;
        let Some(line) = layout.row_at_y(y).map(|r| r.index) else { return false };
        self.toggle_fold(line, cx)
    }
}
