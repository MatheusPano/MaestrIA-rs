use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    None,
    Typing,
    Erasing,
}

#[derive(Clone, Debug)]
struct Snapshot {
    text: String,
    head: usize,
    anchor: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Punct,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

#[derive(Clone, Debug, Default)]
pub struct Buffer {
    text: String,
    head: usize,
    anchor: usize,
    marked: Option<Range<usize>>,
    version: u64,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    group: Option<Group>,
}

impl Buffer {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Default::default() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn head(&self) -> usize {
        self.head
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    pub fn reset(&mut self, text: impl Into<String>) {
        *self = Self { text: text.into(), version: self.version + 1, ..Default::default() };
    }

    pub fn selection(&self) -> Range<usize> {
        self.head.min(self.anchor)..self.head.max(self.anchor)
    }

    pub fn reversed(&self) -> bool {
        self.head < self.anchor
    }

    pub fn has_selection(&self) -> bool {
        self.head != self.anchor
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    fn snap(&self, at: usize) -> usize {
        let mut at = at.min(self.text.len());
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    pub fn set_cursor(&mut self, at: usize, extend: bool) {
        self.head = self.snap(at);
        if !extend {
            self.anchor = self.head;
        }
        self.group = None;
    }

    pub fn select(&mut self, range: Range<usize>) {
        self.anchor = self.snap(range.start);
        self.head = self.snap(range.end);
        self.group = None;
    }

    pub fn select_all(&mut self) {
        self.select(0..self.text.len());
    }

    fn checkpoint(&mut self, group: Group) {
        if group == Group::None || self.group != Some(group) {
            self.undo.push(Snapshot { text: self.text.clone(), head: self.head, anchor: self.anchor });
            if self.undo.len() > 500 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.group = Some(group);
    }

    fn splice(&mut self, range: Range<usize>, new: &str) -> usize {
        let range = self.snap(range.start)..self.snap(range.end);
        self.text.replace_range(range.clone(), new);
        self.version += 1;
        range.start + new.len()
    }

    pub fn replace(&mut self, range: Range<usize>, new: &str) {
        let group = if new.chars().count() == 1 && !new.contains(char::is_whitespace) {
            Group::Typing
        } else {
            Group::None
        };
        self.checkpoint(group);
        let end = self.splice(range, new);
        self.head = end;
        self.anchor = end;
        self.marked = None;
    }

    pub fn insert(&mut self, new: &str) {
        let range = self.marked.clone().unwrap_or_else(|| self.selection());
        self.replace(range, new);
    }

    pub fn replace_and_mark(&mut self, range: Range<usize>, new: &str, select: Option<Range<usize>>) {
        self.checkpoint(Group::Typing);
        let start = self.snap(range.start);
        let end = self.splice(range, new);
        self.marked = (!new.is_empty()).then_some(start..end);
        match select {
            Some(sel) => {
                self.anchor = self.snap(start + sel.start);
                self.head = self.snap(start + sel.end);
            }
            None => {
                self.head = end;
                self.anchor = end;
            }
        }
    }

    pub fn unmark(&mut self) {
        self.marked = None;
    }

    fn erase(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        self.checkpoint(Group::Erasing);
        let at = self.splice(range, "");
        self.head = at;
        self.anchor = at;
    }

    pub fn backspace(&mut self, word: bool) {
        let range = if self.has_selection() {
            self.selection()
        } else if word {
            self.word_before(self.head)..self.head
        } else {
            self.prev(self.head)..self.head
        };
        self.erase(range);
    }

    pub fn delete(&mut self, word: bool) {
        let range = if self.has_selection() {
            self.selection()
        } else if word {
            self.head..self.word_after(self.head)
        } else {
            self.head..self.next(self.head)
        };
        self.erase(range);
    }

    pub fn cut(&mut self) -> Option<String> {
        if !self.has_selection() {
            return None;
        }
        let out = self.selected_text().to_owned();
        self.checkpoint(Group::None);
        let at = self.splice(self.selection(), "");
        self.head = at;
        self.anchor = at;
        Some(out)
    }

    pub fn undo(&mut self) -> bool {
        self.restore(true)
    }

    pub fn redo(&mut self) -> bool {
        self.restore(false)
    }

    fn restore(&mut self, back: bool) -> bool {
        let (from, to) = if back { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
        let Some(snap) = from.pop() else {
            return false;
        };
        to.push(Snapshot { text: std::mem::take(&mut self.text), head: self.head, anchor: self.anchor });
        self.text = snap.text;
        self.head = snap.head;
        self.anchor = snap.anchor;
        self.marked = None;
        self.group = None;
        self.version += 1;
        true
    }

    pub fn prev(&self, at: usize) -> usize {
        self.text[..at].char_indices().next_back().map_or(0, |(i, _)| i)
    }

    pub fn next(&self, at: usize) -> usize {
        self.text[at..].chars().next().map_or(at, |c| at + c.len_utf8())
    }

    pub fn word_before(&self, at: usize) -> usize {
        let trimmed = self.text[..at].trim_end();
        let Some(last) = trimmed.chars().next_back() else {
            return 0;
        };
        let kind = class(last);
        let mut start = trimmed.len();
        for (i, c) in trimmed.char_indices().rev() {
            if class(c) != kind {
                break;
            }
            start = i;
        }
        start
    }

    pub fn word_after(&self, at: usize) -> usize {
        let rest = &self.text[at..];
        let skipped = rest.len() - rest.trim_start().len();
        let mut end = at + skipped;
        let kind = self.text[end..].chars().next().map(class);
        for c in self.text[end..].chars() {
            if Some(class(c)) != kind {
                break;
            }
            end += c.len_utf8();
        }
        end
    }

    pub fn word_at(&self, at: usize) -> Range<usize> {
        let at = self.snap(at);
        let Some(c) = self.text[at..].chars().next().or_else(|| self.text[..at].chars().next_back()) else {
            return at..at;
        };
        let kind = class(c);
        let mut start = at;
        for (i, ch) in self.text[..at].char_indices().rev() {
            if class(ch) != kind {
                break;
            }
            start = i;
        }
        let mut end = at;
        for ch in self.text[at..].chars() {
            if class(ch) != kind {
                break;
            }
            end += ch.len_utf8();
        }
        start..end
    }

    pub fn line_start(&self, at: usize) -> usize {
        self.text[..at].rfind('\n').map_or(0, |i| i + 1)
    }

    pub fn line_end(&self, at: usize) -> usize {
        self.text[at..].find('\n').map_or(self.text.len(), |i| at + i)
    }

    pub fn line_at(&self, at: usize) -> Range<usize> {
        let end = self.line_end(at);
        self.line_start(at)..self.next(end)
    }

    pub fn smart_home(&self, at: usize) -> usize {
        let start = self.line_start(at);
        let line = &self.text[start..self.line_end(at)];
        let indent = start + line.len() - line.trim_start().len();
        if at == indent { start } else { indent }
    }

    pub fn vertical(&self, at: usize, down: bool) -> usize {
        let start = self.line_start(at);
        let column = self.text[start..at].chars().count();
        let target = if down {
            let end = self.line_end(at);
            if end == self.text.len() {
                return end;
            }
            end + 1
        } else {
            if start == 0 {
                return 0;
            }
            self.line_start(start - 1)
        };
        let end = self.line_end(target);
        self.text[target..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(i, _)| target + i)
    }

    pub fn to_utf16(&self, at: usize) -> usize {
        self.text[..self.snap(at)].encode_utf16().count()
    }

    pub fn offset_of_utf16(&self, units: usize) -> usize {
        let mut seen = 0;
        for (i, c) in self.text.char_indices() {
            if seen >= units {
                return i;
            }
            seen += c.len_utf16();
        }
        self.text.len()
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.to_utf16(range.start)..self.to_utf16(range.end)
    }

    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_of_utf16(range.start)..self.offset_of_utf16(range.end)
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
