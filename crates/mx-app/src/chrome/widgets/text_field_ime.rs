use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, point, px, size};

use super::{TextField, byte_at, units_at};

impl TextField {
    fn utf16(&self, r: &Range<usize>) -> Range<usize> {
        units_at(&self.text, r.start)..units_at(&self.text, r.end)
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let r = byte_at(&self.text, range.start)..byte_at(&self.text, range.end);
        adjusted.replace(self.utf16(&r));
        Some(self.text[r].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.utf16(&self.selection()), reversed: false })
    }

    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.utf16(r))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text.chars().any(|c| c.is_control() && c != '\n') {
            return;
        }
        let target = self.target(range);
        self.replace(target, text);
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.target(range);
        let placed = self.replace(target, text);
        if !placed.is_empty() {
            self.marked = Some(placed.clone());
        }
        if let Some(sel) = selected {
            self.cursor = placed.start + byte_at(&self.text[placed.clone()], sel.end);
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let b = if self.bounds.size.width > px(0.) { self.bounds } else { element_bounds };
        Some(Bounds::new(point(b.origin.x + px(12.), b.origin.y + px(10.)), size(px(1.), px(18.))))
    }

    fn character_index_for_point(&mut self, _at: Point<Pixels>, _window: &mut Window, _cx: &mut Context<Self>) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext, EntityInputHandler, TestAppContext};

    use super::super::TextField;

    #[gpui::test]
    fn a_dead_key_composes_into_one_accented_letter(cx: &mut TestAppContext) {
        let field = cx.new(TextField::new);
        let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
        field.update_in(cx, |f, window, cx| {
            f.replace_text_in_range(None, "S", window, cx);
            f.replace_and_mark_text_in_range(None, "~", None, window, cx);
            f.replace_text_in_range(None, "ã", window, cx);
            f.replace_text_in_range(None, "o", window, cx);
        });
        assert_eq!(field.read_with(cx, |f, _| f.text().to_owned()), "São");
    }

    #[gpui::test]
    fn ranges_are_counted_in_utf16_units(cx: &mut TestAppContext) {
        let field = cx.new(|cx| TextField::new(cx).with_text("é😀x"));
        let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
        field.update_in(cx, |f, window, cx| {
            let mut adjusted = None;
            assert_eq!(f.text_for_range(1..3, &mut adjusted, window, cx).as_deref(), Some("😀"));
            assert_eq!(f.selected_text_range(false, window, cx).unwrap().range, 4..4);
            f.replace_text_in_range(Some(1..3), "", window, cx);
        });
        assert_eq!(field.read_with(cx, |f, _| f.text().to_owned()), "éx");
    }

    #[gpui::test]
    fn select_all_then_typing_replaces_everything(cx: &mut TestAppContext) {
        let field = cx.new(|cx| TextField::new(cx).with_text("velho"));
        let (_, cx) = cx.add_window_view(|_, _| gpui::Empty);
        field.update_in(cx, |f, window, cx| {
            f.select_all(cx);
            f.replace_text_in_range(None, "n", window, cx);
        });
        assert_eq!(field.read_with(cx, |f, _| f.text().to_owned()), "n");
    }
}
