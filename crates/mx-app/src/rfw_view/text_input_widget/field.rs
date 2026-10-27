use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, Element, ElementId, ElementInputHandler, Entity, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, LayoutId, Pixels, Point, SharedString, Style, TextAlign, TextRun, Window, WrappedLine, fill, font,
    point, px, relative, size,
};

use super::FieldView;
use crate::rfw_view::style::UI_FONT;

pub const LINE: f32 = 1.35;

pub struct Row {
    pub start: usize,
    pub y: Pixels,
    pub height: Pixels,
    pub line: WrappedLine,
}

pub struct FieldLayout {
    pub rows: Vec<Row>,
    pub line_height: Pixels,
    pub height: Pixels,
    pub width: Pixels,
    pub version: u64,
    pub font_size: Pixels,
}

impl FieldLayout {
    pub fn row_of(&self, at: usize) -> usize {
        self.rows.partition_point(|r| r.start <= at).saturating_sub(1)
    }

    pub fn position(&self, at: usize) -> Point<Pixels> {
        let Some(row) = self.rows.get(self.row_of(at)) else { return point(px(0.), px(0.)) };
        let local = (at - row.start).min(row.line.len());
        let p = row.line.position_for_index(local, self.line_height).unwrap_or_else(|| point(px(0.), px(0.)));
        point(p.x, p.y + row.y)
    }

    pub fn offset_at(&self, p: Point<Pixels>) -> usize {
        let Some(last) = self.rows.last() else { return 0 };
        if p.y >= last.y + last.height {
            return last.start + last.line.len();
        }
        let i = self.rows.partition_point(|r| r.y <= p.y).saturating_sub(1);
        let row = &self.rows[i];
        let y = (p.y - row.y).clamp(px(0.), row.height - px(1.));
        let local = match row.line.closest_index_for_position(point(p.x.max(px(0.)), y), self.line_height) {
            Ok(i) | Err(i) => i,
        };
        row.start + local.min(row.line.len())
    }
}

fn run(len: usize, color: Hsla) -> TextRun {
    TextRun { len, font: font(UI_FONT), color, background_color: None, underline: None, strikethrough: None }
}

pub fn line_height(font_size: f32) -> Pixels {
    px((font_size * LINE).round())
}

pub struct Shown {
    pub text: String,
    pub font_size: Pixels,
    pub color: Hsla,
    pub line_height: Pixels,
}

pub fn shape(shown: &Shown, width: Pixels, wrap: bool, version: u64, window: &Window) -> FieldLayout {
    let (text, font_size, color, line_height) = (&shown.text, shown.font_size, shown.color, shown.line_height);
    let mut rows = Vec::new();
    let mut y = px(0.);
    let mut start = 0;
    for raw in text.split('\n') {
        let shown: String = raw.chars().map(|c| if c == '\t' { ' ' } else { c }).collect();
        let line = window
            .text_system()
            .shape_text(SharedString::from(shown), font_size, &[run(raw.len(), color)], wrap.then_some(width), None)
            .ok()
            .and_then(|mut lines| lines.pop())
            .unwrap_or_default();
        let height = line_height * (line.wrap_boundaries.len() + 1) as f32;
        rows.push(Row { start, y, height, line });
        y += height;
        start += raw.len() + 1;
    }
    FieldLayout { rows, line_height, height: y, width, version, font_size }
}

pub struct FieldElement {
    view: Entity<FieldView>,
}

impl FieldElement {
    pub fn new(view: Entity<FieldView>) -> Self {
        Self { view }
    }
}

impl IntoElement for FieldElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

fn highlight(layout: &FieldLayout, range: Range<usize>, origin: Point<Pixels>, color: Hsla, window: &mut Window) {
    if range.is_empty() {
        return;
    }
    let lh = layout.line_height;
    let (first, last) = (layout.row_of(range.start), layout.row_of(range.end));
    for (i, row) in layout.rows.iter().enumerate().take(last + 1).skip(first) {
        let from = if i == first { range.start - row.start } else { 0 };
        let to = if i == last { range.end - row.start } else { row.line.len() };
        let a = row.line.position_for_index(from.min(row.line.len()), lh).unwrap_or_default();
        let b = row.line.position_for_index(to.min(row.line.len()), lh).unwrap_or_default();
        let mut y = a.y;
        while y <= b.y {
            let x0 = if y == a.y { a.x } else { px(0.) };
            let x1 = if y == b.y { b.x } else { layout.width };
            let x1 = if i != last && y == b.y { x1 + layout.font_size * 0.4 } else { x1 };
            let quad = Bounds::new(point(origin.x + x0, origin.y + row.y + y), size((x1 - x0).max(px(0.)), lh));
            window.paint_quad(fill(quad, color));
            y += lh;
        }
    }
}

impl Element for FieldElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Rc<FieldLayout>>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let view = self.view.read(cx);
        let shown = view.shown();
        let (lines, multiline) = (view.spec.lines, view.spec.multiline);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let id = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.unwrap_or(match available.width {
                gpui::AvailableSpace::Definite(w) => w,
                _ => px(10_000.),
            });
            let layout = shape(&shown, width, multiline, 0, window);
            let rows: usize = layout.rows.iter().map(|r| r.line.wrap_boundaries.len() + 1).sum();
            let shown = rows.clamp(1, lines.max(1) as usize) as f32;
            size(width, layout.line_height * shown)
        });
        (id, ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Self::PrepaintState {
        let view = self.view.read(cx);
        let version = view.buffer.version();
        let shown = view.shown();
        let fresh = view.layout.as_ref().filter(|l| {
            l.version == version && l.width == bounds.size.width && l.font_size == shown.font_size
        });
        let layout = match fresh {
            Some(l) if !view.buffer.text().is_empty() => l.clone(),
            _ => Rc::new(shape(&shown, bounds.size.width, view.spec.multiline, version, window)),
        };
        self.view.update(cx, |v, _| v.adopt(layout.clone(), bounds));
        Some(layout)
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), prepaint: &mut Self::PrepaintState, window: &mut Window, cx: &mut App) {
        let Some(layout) = prepaint.take() else { return };
        let view = self.view.read(cx);
        let focus = view.focus.clone();
        let empty = view.buffer.text().is_empty();
        let (selection, head, scroll) = (view.buffer.selection(), view.buffer.head(), view.scroll_y);
        let colors = view.colors;
        let editable = !view.spec.disabled;
        if editable {
            window.handle_input(&focus, ElementInputHandler::new(bounds, self.view.clone()), cx);
        }
        let origin = point(bounds.origin.x, bounds.origin.y - scroll);
        let lh = layout.line_height;
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            if !empty {
                highlight(&layout, selection, origin, colors.selection, window);
            }
            for row in &layout.rows {
                let at = point(origin.x, origin.y + row.y);
                row.line.paint(at, lh, TextAlign::Left, None, window, cx).ok();
            }
            if focus.is_focused(window) && editable {
                let p = if empty { point(px(0.), px(0.)) } else { layout.position(head) };
                let caret = Bounds::new(point(origin.x + p.x, origin.y + p.y + px(2.)), size(px(1.5), lh - px(4.)));
                window.paint_quad(fill(caret, colors.caret));
            }
        });
    }
}
