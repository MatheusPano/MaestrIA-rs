use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, ClipboardItem, FocusHandle, HighlightStyle, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Pixels, Point, SharedString, Styled, StyledText, TextLayout, TextRun,
    Window, canvas, div,
};

const DRAG_START: f32 = 3.;

pub struct Piece {
    pub text: SharedString,
    pub layout: TextLayout,
    pub bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Caret {
    pub piece: usize,
    pub offset: usize,
}

#[derive(Default)]
pub struct SelectionState {
    pub anchor: Option<Caret>,
    pub head: Option<Caret>,
    pieces: Vec<Piece>,
    building: Vec<Piece>,
    pressed: Option<Point<Pixels>>,
    dragging: bool,
    pub focus: Option<FocusHandle>,
    pub color: Option<Hsla>,
}

pub type SelectionSlot = Rc<RefCell<SelectionState>>;

pub fn join_between(before: Bounds<Pixels>, after: Bounds<Pixels>) -> &'static str {
    if after.top() >= before.bottom() - gpui::px(2.) { "\n" } else { " " }
}

pub fn joined(pieces: &[(String, Bounds<Pixels>)]) -> String {
    let mut out = String::new();
    let mut last: Option<Bounds<Pixels>> = None;
    for (text, rect) in pieces.iter().filter(|(t, _)| !t.is_empty()) {
        if let Some(before) = last {
            out.push_str(join_between(before, *rect));
        }
        out.push_str(text);
        last = Some(*rect);
    }
    out
}

pub fn highlighted_runs(runs: &[TextRun], range: &Range<usize>, color: Hsla) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len() + 2);
    let mut at = 0;
    for r in runs {
        let (start, end) = (at, at + r.len);
        at = end;
        let cuts = [start, range.start.clamp(start, end), range.end.clamp(start, end), end];
        for w in cuts.windows(2) {
            if w[1] > w[0] {
                let mut piece = r.clone();
                piece.len = w[1] - w[0];
                if w[0] >= range.start && w[1] <= range.end && !range.is_empty() {
                    piece.background_color = Some(color);
                }
                out.push(piece);
            }
        }
    }
    out
}

impl SelectionState {
    pub fn begin_frame(&mut self) {
        self.building.clear();
    }

    pub fn end_frame(&mut self) {
        self.pieces = std::mem::take(&mut self.building);
        let last = self.pieces.len();
        let valid = |c: &Option<Caret>| c.is_none_or(|c| c.piece < last);
        if !valid(&self.anchor) || !valid(&self.head) {
            self.anchor = None;
            self.head = None;
        }
    }

    fn ordered(&self) -> Option<(Caret, Caret)> {
        let (a, h) = (self.anchor?, self.head?);
        (a != h).then(|| if a <= h { (a, h) } else { (h, a) })
    }

    pub fn range_of(&self, piece: usize, len: usize) -> Option<Range<usize>> {
        let (start, end) = self.ordered()?;
        if piece < start.piece || piece > end.piece {
            return None;
        }
        let from = if piece == start.piece { start.offset.min(len) } else { 0 };
        let to = if piece == end.piece { end.offset.min(len) } else { len };
        (to > from).then_some(from..to)
    }

    pub fn selected_text(&self) -> String {
        let parts: Vec<(String, Bounds<Pixels>)> = self
            .pieces
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let range = self.range_of(i, p.text.len())?;
                let rect = p.bounds.get()?;
                Some((p.text.get(range)?.to_owned(), rect))
            })
            .collect();
        joined(&parts)
    }

    pub fn len(&self) -> usize {
        self.building.len()
    }

    pub fn offset_in(&self, piece: usize, at: Point<Pixels>) -> Option<usize> {
        let p = self.pieces.get(piece)?;
        p.bounds.get()?;
        Some(Self::index_in(p, at))
    }

    pub fn has_selection(&self) -> bool {
        self.ordered().is_some()
    }

    fn index_in(p: &Piece, at: Point<Pixels>) -> usize {
        match p.layout.index_for_position(at) {
            Ok(i) | Err(i) => i.min(p.text.len()),
        }
    }

    pub fn hit(&self, at: Point<Pixels>, exact: bool) -> Option<Caret> {
        let placed = self.pieces.iter().enumerate().filter_map(|(i, p)| Some((i, p, p.bounds.get()?)));
        let mut best: Option<(usize, f32)> = None;
        for (i, p, b) in placed {
            if b.contains(&at) {
                return Some(Caret { piece: i, offset: Self::index_in(p, at) });
            }
            if exact {
                continue;
            }
            let dy = if at.y < b.top() { b.top() - at.y } else { (at.y - b.bottom()).max(gpui::px(0.)) };
            let dx = if at.x < b.left() { b.left() - at.x } else { (at.x - b.right()).max(gpui::px(0.)) };
            let d = f32::from(dy) * 1000. + f32::from(dx);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        let (i, _) = best?;
        let p = &self.pieces[i];
        Some(Caret { piece: i, offset: Self::index_in(p, at) })
    }
}

pub fn piece(slot: &SelectionSlot, text: impl Into<SharedString>, runs: Option<Vec<TextRun>>) -> AnyElement {
    let text: SharedString = text.into();
    let bounds = Rc::new(Cell::new(None));
    let mut st = slot.borrow_mut();
    let index = st.building.len();
    let color = st.color.unwrap_or_else(|| gpui::blue().opacity(0.3));
    let range = st.range_of(index, text.len());
    let styled = match (runs, range) {
        (Some(runs), Some(r)) => StyledText::new(text.clone()).with_runs(highlighted_runs(&runs, &r, color)),
        (Some(runs), None) => StyledText::new(text.clone()).with_runs(runs),
        (None, Some(r)) => StyledText::new(text.clone())
            .with_highlights([(r, HighlightStyle { background_color: Some(color), ..Default::default() })]),
        (None, None) => StyledText::new(text.clone()),
    };
    st.building.push(Piece { text, layout: styled.layout().clone(), bounds: bounds.clone() });
    drop(st);
    let probe = canvas(move |b, _, _| bounds.set(Some(b)), |_, _, _, _| {}).absolute().top_0().left_0().size_full();
    div().relative().child(styled).child(probe).into_any_element()
}

pub type Notify = Rc<dyn Fn(&mut App)>;

pub fn area<E: InteractiveElement + Sized>(mut el: E, slot: &SelectionSlot, notify: Notify) -> E {
    if let Some(focus) = slot.borrow().focus.clone() {
        el = el.track_focus(&focus);
    }
    let (down, moving, up, keys) = (slot.clone(), slot.clone(), slot.clone(), slot.clone());
    let (n_down, n_move) = (notify.clone(), notify);
    el.on_mouse_down(MouseButton::Left, move |e, window: &mut Window, cx: &mut App| {
        let mut st = down.borrow_mut();
        let hit = st.hit(e.position, true);
        st.pressed = Some(e.position);
        st.dragging = false;
        if st.has_selection() || hit.is_some() {
            st.anchor = hit;
            st.head = hit;
        }
        if hit.is_some() {
            let focus = st.focus.get_or_insert_with(|| cx.focus_handle()).clone();
            window.focus(&focus);
        }
        drop(st);
        n_down(cx);
    })
    .on_mouse_move(move |e, window: &mut Window, cx: &mut App| {
        if e.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let mut st = moving.borrow_mut();
        let Some(from) = st.pressed else { return };
        if !st.dragging {
            let d = e.position - from;
            if f32::from(d.x).abs().max(f32::from(d.y).abs()) < DRAG_START {
                return;
            }
            let Some(start) = st.hit(from, false) else { return };
            st.dragging = true;
            st.anchor = Some(start);
            let focus = st.focus.get_or_insert_with(|| cx.focus_handle()).clone();
            window.focus(&focus);
        }
        st.head = st.hit(e.position, false);
        drop(st);
        n_move(cx);
    })
    .on_mouse_up(MouseButton::Left, move |_, _, _| {
        let mut st = up.borrow_mut();
        st.pressed = None;
        st.dragging = false;
    })
    .on_key_down(move |e: &KeyDownEvent, _, cx: &mut App| {
        let k = &e.keystroke;
        let cmd = k.modifiers.control || k.modifiers.platform;
        if cmd && k.key == "c" {
            let text = keys.borrow().selected_text();
            if !text.is_empty() {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                cx.stop_propagation();
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use gpui::{point, px, size};

    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    #[test]
    fn a_piece_below_the_previous_one_starts_a_new_line_and_one_beside_it_gets_a_space() {
        assert_eq!(join_between(rect(0., 0., 10., 20.), rect(0., 20., 10., 20.)), "\n");
        assert_eq!(join_between(rect(0., 0., 10., 20.), rect(0., 18.5, 10., 20.)), "\n");
        assert_eq!(join_between(rect(0., 0., 10., 20.), rect(12., 0., 10., 20.)), " ");
    }

    #[test]
    fn sel1_paragraphs_lines_and_bullets_join_by_position() {
        let pieces = vec![
            ("primeiro parágrafo".to_owned(), rect(0., 0., 200., 20.)),
            ("segundo parágrafo".to_owned(), rect(0., 31., 200., 20.)),
            ("no meio".to_owned(), rect(0., 60., 100., 16.)),
            (String::new(), rect(0., 70., 10., 10.)),
            ("•".to_owned(), rect(0., 80., 10., 20.)),
            ("um item".to_owned(), rect(22., 80., 100., 20.)),
            ("•".to_owned(), rect(0., 102., 10., 20.)),
            ("outro item".to_owned(), rect(22., 102., 100., 20.)),
        ];
        let text = joined(&pieces);
        assert!(text.contains("primeiro parágrafo\nsegundo parágrafo\nno meio\n"), "{text}");
        assert!(text.contains("• um item\n• outro item"), "{text}");
    }

    #[test]
    fn the_selected_range_spans_whole_middle_pieces_and_cuts_the_ends() {
        let mut st = SelectionState { anchor: Some(Caret { piece: 2, offset: 3 }), head: Some(Caret { piece: 0, offset: 5 }), ..Default::default() };
        assert_eq!(st.range_of(0, 10), Some(5..10));
        assert_eq!(st.range_of(1, 4), Some(0..4));
        assert_eq!(st.range_of(2, 9), Some(0..3));
        assert_eq!(st.range_of(3, 9), None);
        st.head = st.anchor;
        assert!(!st.has_selection());
    }

    #[test]
    fn the_selection_background_splits_runs_at_the_selection_edges() {
        let run = TextRun { len: 6, font: gpui::font("x"), color: gpui::black(), background_color: None, underline: None, strikethrough: None };
        let runs = highlighted_runs(&[run.clone(), TextRun { len: 4, ..run }], &(3..8), gpui::red());
        let lens: Vec<(usize, bool)> = runs.iter().map(|r| (r.len, r.background_color.is_some())).collect();
        assert_eq!(lens, [(3, false), (3, true), (2, true), (2, false)]);
    }
}
