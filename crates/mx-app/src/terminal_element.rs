use gpui::{
    App, BorderStyle, Bounds, Element, ElementId, Entity, Font, FontStyle, FontWeight,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, PaintQuad, Pixels, Point,
    ShapedLine, SharedString, StrikethroughStyle, Style, TextRun, UnderlineStyle, Window, fill,
    outline, point, px, relative, rgb, size,
};
use mx_term::{CellStyle, CursorKind, GridSize, SnapCell, Snapshot};
use mx_theme::Rgb;

use crate::terminal_view::TerminalView;

pub const FONT_FAMILY: &str = "Hack";
const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT_FACTOR: f32 = 1.3;

#[derive(Clone, Copy, Debug)]
pub struct CellMetrics {
    pub width: Pixels,
    pub height: Pixels,
}

pub struct TerminalElement {
    view: Entity<TerminalView>,
}

impl TerminalElement {
    pub fn new(view: Entity<TerminalView>) -> Self {
        Self { view }
    }
}

pub struct Layout {
    background: Hsla,
    rects: Vec<PaintQuad>,
    lines: Vec<(Point<Pixels>, ShapedLine)>,
    cursor: Option<PaintQuad>,
    line_height: Pixels,
}

fn hsla(c: Rgb) -> Hsla {
    rgb(c.to_u32()).into()
}

fn base_font(bold: bool, italic: bool) -> Font {
    let mut font = gpui::font(FONT_FAMILY);
    font.weight = if bold {
        FontWeight::BOLD
    } else {
        FontWeight::NORMAL
    };
    font.style = if italic {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    font
}

pub fn cell_metrics(window: &Window) -> CellMetrics {
    let text_system = window.text_system();
    let font_size = px(FONT_SIZE);
    let font_id = text_system.resolve_font(&base_font(false, false));
    let width = text_system
        .advance(font_id, font_size, 'm')
        .map(|s| s.width)
        .unwrap_or(px(FONT_SIZE * 0.6));
    CellMetrics {
        width,
        height: px((FONT_SIZE * LINE_HEIGHT_FACTOR).round()),
    }
}

pub fn grid_size(bounds: Bounds<Pixels>, metrics: CellMetrics, scale: f32) -> GridSize {
    let cols = (bounds.size.width / metrics.width).floor().max(2.0) as u16;
    let rows = (bounds.size.height / metrics.height).floor().max(1.0) as u16;
    GridSize {
        cols,
        rows,
        cell_width: (f32::from(metrics.width) * scale).round() as u16,
        cell_height: (f32::from(metrics.height) * scale).round() as u16,
    }
}

struct Batch {
    col: usize,
    text: String,
    style: CellStyle,
    ends_wide: bool,
}

fn batches(row: &[SnapCell]) -> Vec<Batch> {
    let mut out: Vec<Batch> = Vec::new();
    for (col, cell) in row.iter().enumerate() {
        if cell.spacer || cell.c == ' ' && !cell.style.underline && !cell.style.strikeout {
            continue;
        }
        let extend = out.last().is_some_and(|b| {
            !b.ends_wide && b.style == cell.style && b.col + b.text.chars().count() == col
        });
        if extend {
            if let Some(last) = out.last_mut() {
                last.text.push(cell.c);
                last.ends_wide = cell.wide;
            }
        } else {
            out.push(Batch {
                col,
                text: cell.c.to_string(),
                style: cell.style,
                ends_wide: cell.wide,
            });
        }
    }
    out
}

fn layout(
    snap: &Snapshot,
    origin: Point<Pixels>,
    m: CellMetrics,
    focused: bool,
    window: &mut Window,
) -> Layout {
    let cell_origin = |row: usize, col: usize| {
        point(
            origin.x + m.width * col as f32,
            origin.y + m.height * row as f32,
        )
    };
    let mut rects = Vec::new();
    let mut lines = Vec::new();
    let mut cursor = None;
    let font_size = px(FONT_SIZE);

    for row in 0..snap.rows {
        let mut cells = snap.row(row).to_vec();
        if let Some(c) = snap
            .cursor
            .filter(|c| c.row == row && focused && c.kind == CursorKind::Block)
            && let Some(cell) = cells.get_mut(c.col)
        {
            cell.style.bg = c.color;
            cell.style.fg = snap.background;
        }

        let mut col = 0;
        while col < cells.len() {
            let bg = cells[col].style.bg;
            let start = col;
            while col < cells.len() && cells[col].style.bg == bg {
                col += 1;
            }
            if bg != snap.background {
                let size = size(m.width * (col - start) as f32, m.height);
                rects.push(fill(Bounds::new(cell_origin(row, start), size), hsla(bg)));
            }
        }

        for batch in batches(&cells) {
            let color = hsla(batch.style.fg);
            let run = TextRun {
                len: batch.text.len(),
                font: base_font(batch.style.bold, batch.style.italic),
                color,
                background_color: None,
                underline: batch.style.underline.then_some(UnderlineStyle {
                    thickness: px(1.0),
                    color: Some(color),
                    wavy: false,
                }),
                strikethrough: batch.style.strikeout.then_some(StrikethroughStyle {
                    thickness: px(1.0),
                    color: Some(color),
                }),
            };
            let shaped = window.text_system().shape_line(
                SharedString::from(batch.text),
                font_size,
                &[run],
                Some(m.width),
            );
            lines.push((cell_origin(row, batch.col), shaped));
        }
    }

    if let Some(c) = snap.cursor {
        let at = cell_origin(c.row, c.col);
        let color = hsla(c.color);
        cursor = match (c.kind, focused) {
            (CursorKind::Block, true) => None,
            (CursorKind::Beam, true) => Some(fill(Bounds::new(at, size(px(2.0), m.height)), color)),
            (CursorKind::Underline, true) => Some(fill(
                Bounds::new(
                    point(at.x, at.y + m.height - px(2.0)),
                    size(m.width, px(2.0)),
                ),
                color,
            )),
            _ => Some(outline(
                Bounds::new(at, size(m.width, m.height)),
                color,
                BorderStyle::Solid,
            )),
        };
    }

    Layout {
        background: hsla(snap.background),
        rects,
        lines,
        cursor,
        line_height: m.height,
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
    type PrepaintState = Layout;

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
        let metrics = cell_metrics(window);
        let grid = grid_size(bounds, metrics, window.scale_factor());
        let view = self.view.read(cx);
        let focused = view.focus().is_focused(window);
        view.session().resize(grid);
        let snap = view.session().snapshot();
        layout(&snap, bounds.origin, metrics, focused, window)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_quad(fill(bounds, layout.background));
        for rect in layout.rects.drain(..) {
            window.paint_quad(rect);
        }
        for (origin, line) in &layout.lines {
            if let Err(err) = line.paint(*origin, layout.line_height, window, cx) {
                log::warn!("terminal line paint failed: {err}");
            }
        }
        if let Some(cursor) = layout.cursor.take() {
            window.paint_quad(cursor);
        }
    }
}
