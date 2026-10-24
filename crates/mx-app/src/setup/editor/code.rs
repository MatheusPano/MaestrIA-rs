use std::ops::Range;

use gpui::{Context, px};

use super::{EditorEvent, TextEditor};

impl TextEditor {
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    #[cfg(test)]
    pub fn is_focused(&self, window: &gpui::Window) -> bool {
        self.focus.is_focused(window)
    }

    pub fn cursor_position(&self) -> (usize, usize, usize) {
        let text = self.buffer.text();
        let head = self.buffer.head();
        let start = self.buffer.line_start(head);
        let line = text[..start].matches('\n').count() + 1;
        let column = text[start..head].chars().count() + 1;
        let selected = self.buffer.selected_text().chars().count();
        (line, column, selected)
    }

    pub fn offset_of(&self, line: usize, column: usize) -> usize {
        let text = self.buffer.text();
        let start = if line <= 1 {
            0
        } else {
            text.match_indices('\n').nth(line - 2).map_or(text.len(), |(i, _)| i + 1)
        };
        let end = self.buffer.line_end(start);
        text[start..end].char_indices().nth(column.saturating_sub(1)).map_or(end, |(i, _)| start + i)
    }

    pub fn go_to(&mut self, line: usize, column: usize, cx: &mut Context<Self>) {
        let at = self.offset_of(line, column);
        self.buffer.set_cursor(at, false);
        self.goal_x = None;
        self.center_next = true;
        self.moved(cx);
    }

    pub fn reload(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let head = self.buffer.head();
        self.buffer.reset(text);
        self.buffer.set_cursor(head, false);
        self.goal_x = None;
        self.found.clear();
        self.found_current = None;
        cx.notify();
    }

    pub fn set_found(&mut self, found: Vec<Range<usize>>, current: Option<usize>, cx: &mut Context<Self>) {
        if let Some(r) = current.and_then(|i| found.get(i)) {
            self.buffer.select(r.clone());
            self.goal_x = None;
            self.center_next = !self.visible(r.start);
            self.autoscroll = true;
            cx.emit(EditorEvent::Cursor);
        }
        self.found = found;
        self.found_current = current;
        cx.notify();
    }

    fn visible(&self, at: usize) -> bool {
        let Some(layout) = &self.layout else { return false };
        let y = layout.position(at).y + px(super::PAD_TOP);
        y >= self.scroll_y && y + layout.line_height <= self.scroll_y + self.bounds.size.height
    }

    pub fn selection(&self) -> Range<usize> {
        self.buffer.selection()
    }

    pub fn selected_text(&self) -> &str {
        self.buffer.selected_text()
    }

    pub fn replace_range(&mut self, range: Range<usize>, with: &str, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.buffer.replace(range, with);
        self.edited(cx);
    }

    pub fn replace_all(&mut self, ranges: &[Range<usize>], with: &str, cx: &mut Context<Self>) {
        if self.read_only || ranges.is_empty() {
            return;
        }
        let text = self.buffer.text();
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for r in ranges {
            out.push_str(&text[last..r.start]);
            out.push_str(with);
            last = r.end;
        }
        out.push_str(&text[last..]);
        let head = self.buffer.head().min(out.len());
        let len = text.len();
        self.buffer.replace(0..len, &out);
        self.buffer.set_cursor(head, false);
        self.edited(cx);
    }
}
