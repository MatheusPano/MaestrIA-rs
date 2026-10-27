use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, size};

use super::FieldView;

impl EntityInputHandler for FieldView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.range_from_utf16(&range);
        adjusted.replace(self.buffer.range_to_utf16(&range));
        Some(self.buffer.text()[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(&self.buffer.selection()),
            reversed: self.buffer.reversed(),
        })
    }

    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer.marked().map(|r| self.buffer.range_to_utf16(&r))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.buffer.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.disabled {
            return;
        }
        match range {
            Some(r) => {
                let r = self.buffer.range_from_utf16(&r);
                self.buffer.replace(r, text);
            }
            None => self.buffer.insert(text),
        }
        self.edited(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.disabled {
            return;
        }
        let range = range
            .map(|r| self.buffer.range_from_utf16(&r))
            .or_else(|| self.buffer.marked())
            .unwrap_or_else(|| self.buffer.selection());
        let selected = selected.map(|r| byte_of(text, r.start)..byte_of(text, r.end));
        self.buffer.replace_and_mark(range, text, selected);
        self.edited(cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let start = self.buffer.offset_of_utf16(range.start);
        let at = self.to_window(layout.position(start));
        Some(Bounds::new(at, size(Pixels::from(1.), layout.line_height)))
    }

    fn character_index_for_point(
        &mut self,
        at: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let offset = self.offset_at_window(at)?;
        Some(self.buffer.to_utf16(offset))
    }
}

fn byte_of(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (i, c) in text.char_indices() {
        if seen >= units {
            return i;
        }
        seen += c.len_utf16();
    }
    text.len()
}
