use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, Element, ElementId, ElementInputHandler, Entity, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, LayoutId, Pixels, SharedString, Style, TextAlign, TextRun, Window, WrappedLine, fill, font, point,
    px, relative, size,
};

use super::layout::{EditorLayout, Row};
use super::syntax::{self, Spans};
use super::{EditorStyle, PAD_LEFT, PAD_RIGHT, PAD_TOP, TextEditor};
use crate::chrome::style::MONO;

const THUMB: f32 = 6.;
const GUTTER_GAP: f32 = 14.;

pub struct EditorElement {
    editor: Entity<TextEditor>,
}

impl EditorElement {
    pub fn new(editor: Entity<TextEditor>) -> Self {
        Self { editor }
    }
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn run(len: usize, color: Hsla) -> TextRun {
    TextRun { len, font: font(MONO), color, background_color: None, underline: None, strikethrough: None }
}

fn runs(line: Range<usize>, spans: &Spans, next: &mut usize, style: &EditorStyle) -> Vec<TextRun> {
    let mut out = Vec::new();
    let Some(colors) = &style.syntax else {
        return vec![run(line.len(), style.fg)];
    };
    let mut at = line.start;
    while *next < spans.len() && spans[*next].0.end <= line.start {
        *next += 1;
    }
    let mut i = *next;
    while let Some((r, tone)) = spans.get(i) {
        if r.start >= line.end {
            break;
        }
        let s = r.start.max(line.start);
        let e = r.end.min(line.end);
        if s > at {
            out.push(run(s - at, style.fg));
        }
        if e > s {
            out.push(run(e - s, colors.of(*tone)));
        }
        at = at.max(e);
        if r.end > line.end {
            break;
        }
        i += 1;
    }
    *next = i;
    if line.end > at || out.is_empty() {
        out.push(run(line.end - at, style.fg));
    }
    out
}

fn digit_width(font_size: Pixels, window: &Window) -> Pixels {
    let ts = window.text_system();
    let id = ts.resolve_font(&font(MONO));
    ts.advance(id, font_size, '0').map_or(font_size * 0.6, |s| s.width)
}

fn shape(editor: &TextEditor, width: Pixels, window: &mut Window) -> EditorLayout {
    let style = &editor.style;
    let font_size = px(style.font_size);
    let line_height = editor.line_height();
    let text = editor.buffer.text();
    let gutter = match style.gutter {
        Some(_) => {
            let digits = (text.matches('\n').count() + 1).to_string().len().max(2);
            (digit_width(font_size, window) * digits as f32 + px(GUTTER_GAP)).round()
        }
        None => px(0.),
    };
    let wrap_width = (width - px(PAD_LEFT + PAD_RIGHT) - gutter).max(px(40.));
    let wrap = style.wrap.then_some(wrap_width);
    let spans = if style.syntax.is_some() { syntax::highlight(text, &style.language) } else { Vec::new() };
    let mut next = 0;
    let mut rows = Vec::new();
    let mut y = px(0.);
    let mut widest = px(0.);
    let mut start = 0;
    for raw in text.split('\n') {
        let shown: String = raw.chars().map(|c| if c == '\t' { ' ' } else { c }).collect();
        let runs = runs(start..start + raw.len(), &spans, &mut next, style);
        let line = window
            .text_system()
            .shape_text(SharedString::from(shown), font_size, &runs, wrap, None)
            .ok()
            .and_then(|mut lines| lines.pop())
            .unwrap_or_default();
        widest = widest.max(line.unwrapped_layout.width);
        let height = line_height * (line.wrap_boundaries.len() + 1) as f32;
        rows.push(Row { start, y, height, line });
        y += height;
        start += raw.len() + 1;
    }
    EditorLayout {
        rows,
        line_height,
        height: y,
        width: if style.wrap { wrap_width } else { widest },
        wrap_width,
        gutter,
        font_size,
        version: editor.buffer.version(),
    }
}

fn segments(line: &WrappedLine) -> Vec<(usize, usize, Pixels)> {
    let layout = &line.unwrapped_layout;
    let mut cuts: Vec<usize> = line
        .wrap_boundaries
        .iter()
        .map(|b| layout.runs[b.run_ix].glyphs[b.glyph_ix].index)
        .collect();
    cuts.push(line.len());
    let mut out = Vec::with_capacity(cuts.len());
    let mut from = 0;
    for to in cuts {
        out.push((from, to, layout.x_for_index(from)));
        from = to;
    }
    out
}

fn highlight(layout: &EditorLayout, range: Range<usize>, origin: gpui::Point<Pixels>, color: Hsla, band: Option<Pixels>, window: &mut Window) {
    if range.is_empty() {
        return;
    }
    let lh = layout.line_height;
    let first = layout.row_of(range.start);
    let last = layout.row_of(range.end);
    for row in &layout.rows[first..=last] {
        let len = row.line.len();
        let from = range.start.saturating_sub(row.start).min(len);
        let to = range.end.saturating_sub(row.start).min(len);
        let past_end = range.end > row.start + len;
        let segs = segments(&row.line);
        let n = segs.len();
        for (k, (a, b, x0)) in segs.into_iter().enumerate() {
            let s = from.max(a);
            let e = to.min(b);
            let tail = past_end && k + 1 == n;
            if s > e || (s == e && !tail) {
                continue;
            }
            let ux = &row.line.unwrapped_layout;
            let x1 = ux.x_for_index(s) - x0;
            let mut x2 = ux.x_for_index(e) - x0;
            if tail {
                x2 += layout.font_size * 0.5;
            }
            let top = origin.y + row.y + lh * k as f32;
            let (y, h) = match band {
                Some(h) => (top + lh - h - px(2.), h),
                None => (top, lh),
            };
            window.paint_quad(fill(Bounds::new(point(origin.x + x1, y), size(x2 - x1, h)), color));
        }
    }
}

fn visible(layout: &EditorLayout, range: &Range<usize>, top: Pixels, bottom: Pixels) -> bool {
    let a = layout.position(range.start).y;
    let b = layout.position(range.end).y;
    b + layout.line_height >= top && a <= bottom
}

fn paint_gutter(layout: &EditorLayout, bounds: Bounds<Pixels>, origin_y: Pixels, current: usize, colors: (Hsla, Hsla), window: &mut Window, cx: &mut App) {
    let lh = layout.line_height;
    let top = bounds.origin.y - origin_y - lh;
    let bottom = top + bounds.size.height + lh * 2.;
    let first = layout.rows.partition_point(|r| r.y + r.height < top);
    for (i, row) in layout.rows.iter().enumerate().skip(first) {
        if row.y > bottom {
            break;
        }
        let color = if i == current { colors.1 } else { colors.0 };
        let label = SharedString::from((i + 1).to_string());
        let shaped = window.text_system().shape_line(label.clone(), layout.font_size, &[run(label.len(), color)], None);
        let x = bounds.origin.x + layout.gutter - px(GUTTER_GAP / 2.) - shaped.width;
        shaped.paint(point(x, origin_y + row.y), lh, window, cx).ok();
    }
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Rc<EditorLayout>>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.editor.read(cx);
        let gutter = editor.layout.as_ref().map_or(px(0.), |l| l.gutter);
        let wrap_width = (bounds.size.width - px(PAD_LEFT + PAD_RIGHT) - gutter).max(px(40.));
        let fresh = editor.layout.as_ref().filter(|l| {
            l.version == editor.buffer.version()
                && (l.wrap_width == wrap_width || !editor.style.wrap)
                && l.font_size == px(editor.style.font_size)
                && l.line_height == editor.line_height()
        });
        let layout = match fresh {
            Some(l) => l.clone(),
            None => Rc::new(shape(editor, bounds.size.width, window)),
        };
        self.editor.update(cx, |e, _| e.adopt_layout(layout.clone(), bounds));
        Some(layout)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(layout) = prepaint.take() else {
            return;
        };
        let editor = self.editor.read(cx);
        let focus = editor.focus.clone();
        let style = editor.style.clone();
        let selection = editor.buffer.selection();
        let marked = editor.buffer.marked();
        let head = editor.buffer.head();
        let scroll = editor.scroll_y;
        let scroll_x = editor.scroll_x;
        let max_scroll = editor.max_scroll();
        let found = editor.found.clone();
        let found_current = editor.found_current;
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.editor.clone()), cx);
        let origin = point(bounds.origin.x + px(PAD_LEFT) + layout.gutter - scroll_x, bounds.origin.y + px(PAD_TOP) - scroll);
        let lh = layout.line_height;
        let top = scroll - px(PAD_TOP);
        let bottom = top + bounds.size.height;
        if let Some(colors) = style.gutter {
            window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                let current = layout.row_of(head);
                paint_gutter(&layout, bounds, origin.y, current, colors, window, cx);
            });
        }
        let text_area = Bounds::new(
            point(bounds.origin.x + layout.gutter, bounds.origin.y),
            size(bounds.size.width - layout.gutter, bounds.size.height),
        );
        window.with_content_mask(Some(gpui::ContentMask { bounds: text_area }), |window| {
            for (i, r) in found.iter().enumerate() {
                if visible(&layout, r, top, bottom) {
                    let color = if Some(i) == found_current { style.found_current } else { style.found };
                    highlight(&layout, r.clone(), origin, color, None, window);
                }
            }
            highlight(&layout, selection, origin, style.selection, None, window);
            if let Some(m) = marked {
                highlight(&layout, m, origin, style.fg, Some(px(1.)), window);
            }
            for row in &layout.rows {
                if row.y + row.height < top || row.y > bottom {
                    continue;
                }
                let at = point(origin.x, origin.y + row.y);
                row.line.paint(at, lh, TextAlign::Left, None, window, cx).ok();
            }
            if focus.is_focused(window) {
                let p = layout.position(head);
                let caret = Bounds::new(point(origin.x + p.x - px(0.5), origin.y + p.y + px(1.)), size(px(1.5), lh - px(2.)));
                window.paint_quad(fill(caret, style.caret));
            }
        });
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            if max_scroll > px(0.) {
                let view = bounds.size.height;
                let content = view + max_scroll;
                let h = (view * (view / content)).max(px(24.));
                let y = bounds.origin.y + (view - h) * (scroll / max_scroll);
                let x = bounds.origin.x + bounds.size.width - px(THUMB + 2.);
                window.paint_quad(fill(Bounds::new(point(x, y), size(px(THUMB), h)), style.thumb).corner_radii(px(THUMB / 2.)));
            }
        });
    }
}
