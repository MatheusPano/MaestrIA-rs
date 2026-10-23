use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, BorderStyle, Bounds, Element, ElementId, ElementInputHandler, Entity, Font, FontFallbacks,
    FontStyle, FontWeight, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId,
    Pixels, Point, ShapedLine, SharedString, StrikethroughStyle, Style, TextRun, UnderlineStyle,
    Window, fill, outline, point, px, relative, rgb, size,
};
use mx_term::{CellStyle, CursorKind, GridSize, SnapCell, Snapshot};
use mx_theme::{Color, Rgb};

use crate::terminal_view::TerminalView;

pub const FONT_FAMILY: &str = "Hack";
pub const FALLBACKS: [&str; 12] = [
    "Menlo",
    "Monaco",
    "STIX Two Math",
    "Apple Symbols",
    "Noto Sans Symbols 2",
    "Noto Sans Symbols",
    "DejaVu Sans",
    "Noto Color Emoji",
    "Liberation Mono",
    "Courier New",
    "monospace",
    "sans-serif",
];
pub const PAD_X: f32 = 12.0;
pub const PAD_Y: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    pub size: f32,
    pub line: f32,
}

impl Default for Typography {
    fn default() -> Self {
        Self { size: 13.0, line: 1.2 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellMetrics {
    pub width: Pixels,
    pub height: Pixels,
}

#[derive(Default)]
struct RowCache {
    cells: Vec<SnapCell>,
    valid: bool,
    bg: Vec<(usize, usize, Rgb)>,
    selected: Vec<(usize, usize)>,
    texts: Vec<(usize, ShapedLine)>,
}

pub struct RenderState {
    pub typography: Typography,
    pub selection: Color,
    pub metrics: Option<(Typography, CellMetrics)>,
    pub origin: Point<Pixels>,
    pub cols: usize,
    pub rows: usize,
    pub painted: bool,
    pub cursor_bounds: Option<Bounds<Pixels>>,
    pub marked: Option<String>,
    snap: Snapshot,
    cache: Vec<RowCache>,
    cursor_text: Option<(Point<Pixels>, ShapedLine)>,
    marked_line: Option<(Point<Pixels>, ShapedLine)>,
}

impl RenderState {
    pub fn new(typography: Typography, selection: Color) -> Self {
        Self {
            typography,
            selection,
            metrics: None,
            origin: Point::default(),
            cols: 80,
            rows: 24,
            painted: false,
            cursor_bounds: None,
            marked: None,
            snap: Snapshot::default(),
            cache: Vec::new(),
            cursor_text: None,
            marked_line: None,
        }
    }

    pub fn cell(&self) -> CellMetrics {
        self.metrics.map(|(_, m)| m).unwrap_or(CellMetrics { width: px(8.), height: px(16.) })
    }

    pub fn cell_at(&self, position: Point<Pixels>) -> (f32, f32, bool) {
        let m = self.cell();
        let x = (position.x - self.origin.x) / m.width;
        let y = (position.y - self.origin.y) / m.height;
        (x.floor(), y.floor(), x.fract() >= 0.5)
    }

    fn invalidate(&mut self) {
        self.cache.iter_mut().for_each(|r| r.valid = false);
    }
}

pub type SharedRender = Rc<RefCell<RenderState>>;

fn hsla(c: Rgb) -> Hsla {
    rgb(c.to_u32()).into()
}

fn color(c: Color) -> Hsla {
    let mut h: Hsla = rgb(c.rgb().to_u32()).into();
    h.a = c.alpha() as f32;
    h
}

pub fn font(bold: bool, italic: bool) -> Font {
    let mut font = gpui::font(FONT_FAMILY);
    font.weight = if bold { FontWeight::BOLD } else { FontWeight::NORMAL };
    font.style = if italic { FontStyle::Italic } else { FontStyle::Normal };
    font.fallbacks = Some(FontFallbacks::from_fonts(FALLBACKS.iter().map(|f| (*f).to_owned()).collect()));
    font
}

fn measure(window: &Window, t: Typography) -> CellMetrics {
    let text_system = window.text_system();
    let font_id = text_system.resolve_font(&font(false, false));
    let width = text_system
        .advance(font_id, px(t.size), 'm')
        .map(|s| s.width)
        .unwrap_or(px(t.size * 0.6));
    CellMetrics { width, height: px((t.size * t.line).ceil()) }
}

fn grid_size(bounds: Bounds<Pixels>, m: CellMetrics, scale: f32) -> GridSize {
    let w = bounds.size.width - px(2. * PAD_X);
    let h = bounds.size.height - px(2. * PAD_Y);
    GridSize {
        cols: (w / m.width).floor().max(2.0) as u16,
        rows: (h / m.height).floor().max(1.0) as u16,
        cell_width: (f32::from(m.width) * scale).round() as u16,
        cell_height: (f32::from(m.height) * scale).round() as u16,
    }
}

fn run(text_len: usize, style: &CellStyle, fg: Rgb) -> TextRun {
    let c = hsla(fg);
    TextRun {
        len: text_len,
        font: font(style.bold, style.italic),
        color: c,
        background_color: None,
        underline: style.underline.then_some(UnderlineStyle { thickness: px(1.0), color: Some(c), wavy: false }),
        strikethrough: style.strikeout.then_some(StrikethroughStyle { thickness: px(1.0), color: Some(c) }),
    }
}

fn rebuild_row(row: &mut RowCache, cells: &[SnapCell], background: Rgb, font_size: Pixels, m: CellMetrics, window: &Window) {
    row.cells.clear();
    row.cells.extend_from_slice(cells);
    row.bg.clear();
    row.selected.clear();
    row.texts.clear();
    let mut col = 0;
    while col < cells.len() {
        let (start, bg) = (col, cells[col].style.bg);
        while col < cells.len() && cells[col].style.bg == bg {
            col += 1;
        }
        if bg != background {
            row.bg.push((start, col - start, bg));
        }
    }
    col = 0;
    while col < cells.len() {
        if !cells[col].selected {
            col += 1;
            continue;
        }
        let start = col;
        while col < cells.len() && cells[col].selected {
            col += 1;
        }
        row.selected.push((start, col - start));
    }
    let mut text = String::new();
    let mut col = 0;
    while col < cells.len() {
        let cell = &cells[col];
        if cell.spacer || cell.c == ' ' && !cell.style.underline && !cell.style.strikeout {
            col += 1;
            continue;
        }
        let start = col;
        text.clear();
        while col < cells.len() {
            let c = &cells[col];
            if c.spacer || c.style != cell.style || c.c == ' ' && !c.style.underline && !c.style.strikeout {
                break;
            }
            text.push(c.c);
            col += 1;
            if c.wide {
                break;
            }
        }
        let shaped = window.text_system().shape_line(
            SharedString::from(text.clone()),
            font_size,
            &[run(text.len(), &cell.style, cell.style.fg)],
            Some(m.width),
        );
        row.texts.push((start, shaped));
    }
    row.valid = true;
}

pub struct TerminalElement {
    view: Entity<TerminalView>,
}

impl TerminalElement {
    pub fn new(view: Entity<TerminalView>) -> Self {
        Self { view }
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = bool;

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
    ) -> bool {
        if bounds.size.width <= px(1.) || bounds.size.height <= px(1.) {
            return false;
        }
        let view = self.view.read(cx);
        let focused = view.focus().is_focused(window);
        let session = view.session().clone();
        let render = view.render().clone();
        let mut st = render.borrow_mut();
        st.painted = true;
        let t = st.typography;
        let m = match st.metrics {
            Some((key, m)) if key == t => m,
            _ => {
                let m = measure(window, t);
                st.metrics = Some((t, m));
                st.invalidate();
                m
            }
        };
        session.resize(grid_size(bounds, m, window.scale_factor()));
        session.take_dirty();
        let st = &mut *st;
        session.snapshot_into(&mut st.snap);
        st.origin = point(bounds.origin.x + px(PAD_X), bounds.origin.y + px(PAD_Y));
        st.cols = st.snap.cols;
        st.rows = st.snap.rows;
        if st.cache.len() != st.rows {
            st.cache.resize_with(st.rows, RowCache::default);
        }
        let font_size = px(t.size);
        for (r, row) in st.cache.iter_mut().enumerate() {
            let cells = st.snap.row(r);
            if !row.valid || row.cells.as_slice() != cells {
                rebuild_row(row, cells, st.snap.background, font_size, m, window);
            }
        }
        st.cursor_text = None;
        st.cursor_bounds = None;
        if let Some(c) = st.snap.cursor {
            let at = point(st.origin.x + m.width * c.col as f32, st.origin.y + m.height * c.row as f32);
            st.cursor_bounds = Some(Bounds::new(at, size(m.width, m.height)));
            let cell = st.snap.row(c.row)[c.col];
            if focused && c.kind == CursorKind::Block && cell.c != ' ' && st.marked.is_none() {
                let text = cell.c.to_string();
                let shaped = window.text_system().shape_line(
                    SharedString::from(text.clone()),
                    font_size,
                    &[run(text.len(), &cell.style, st.snap.background)],
                    Some(m.width),
                );
                st.cursor_text = Some((at, shaped));
            }
        }
        st.marked_line = match (&st.marked, st.cursor_bounds) {
            (Some(text), Some(b)) if !text.is_empty() => {
                let style = CellStyle {
                    fg: st.snap.foreground,
                    bg: st.snap.background,
                    bold: false,
                    italic: false,
                    underline: true,
                    strikeout: false,
                };
                let shaped = window.text_system().shape_line(
                    SharedString::from(text.clone()),
                    font_size,
                    &[run(text.len(), &style, st.snap.foreground)],
                    Some(m.width),
                );
                Some((b.origin, shaped))
            }
            _ => None,
        };
        true
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        visible: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !*visible {
            return;
        }
        let view = self.view.read(cx);
        let focus = view.focus().clone();
        let focused = focus.is_focused(window);
        let render = view.render().clone();
        let st = render.borrow();
        let m = st.cell();
        let origin = st.origin;
        let cell_at = |row: usize, col: usize| point(origin.x + m.width * col as f32, origin.y + m.height * row as f32);
        window.paint_quad(fill(bounds, hsla(st.snap.background)));
        for (r, row) in st.cache.iter().enumerate() {
            for &(col, len, bg) in &row.bg {
                window.paint_quad(fill(Bounds::new(cell_at(r, col), size(m.width * len as f32, m.height)), hsla(bg)));
            }
        }
        let selection = color(st.selection);
        for (r, row) in st.cache.iter().enumerate() {
            for &(col, len) in &row.selected {
                window.paint_quad(fill(Bounds::new(cell_at(r, col), size(m.width * len as f32, m.height)), selection));
            }
        }
        for (r, row) in st.cache.iter().enumerate() {
            for (col, line) in &row.texts {
                if let Err(err) = line.paint(cell_at(r, *col), m.height, window, cx) {
                    log::warn!("terminal line paint failed: {err}");
                }
            }
        }
        if let (Some(c), Some(b)) = (st.snap.cursor, st.cursor_bounds)
            && focused
            && c.kind == CursorKind::Block
            && st.marked.is_none()
        {
            window.paint_quad(fill(b, hsla(c.color)));
        }
        if let Some((at, line)) = &st.cursor_text {
            let _ = line.paint(*at, m.height, window, cx);
        }
        if let (Some(c), Some(b)) = (st.snap.cursor, st.cursor_bounds)
            && st.marked.is_none()
        {
            let col = hsla(c.color);
            let quad = match (c.kind, focused) {
                (CursorKind::Block, true) => None,
                (CursorKind::Beam, true) => Some(fill(Bounds::new(b.origin, size(px(2.0), m.height)), col)),
                (CursorKind::Underline, true) => Some(fill(
                    Bounds::new(point(b.origin.x, b.origin.y + m.height - px(2.0)), size(m.width, px(2.0))),
                    col,
                )),
                _ => Some(outline(b, col, BorderStyle::Solid)),
            };
            if let Some(q) = quad {
                window.paint_quad(q);
            }
        }
        if let Some((at, line)) = &st.marked_line {
            window.paint_quad(fill(Bounds::new(*at, size(line.width, m.height)), hsla(st.snap.background)));
            let _ = line.paint(*at, m.height, window, cx);
        }
        drop(st);
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.view.clone()), cx);
    }
}
