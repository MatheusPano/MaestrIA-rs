use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Style, Styled, Window, point, px, size,
};

use super::layout::El;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Columns {
    Fixed(usize),
    MaxExtent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSpec {
    pub columns: Columns,
    pub main_spacing: f32,
    pub cross_spacing: f32,
    pub ratio: f32,
    pub extent: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cells {
    pub count: usize,
    pub width: f32,
    pub height: f32,
}

impl GridSpec {
    pub fn cells(&self, width: f32) -> Cells {
        let count = match self.columns {
            Columns::Fixed(n) => n.max(1),
            Columns::MaxExtent(max) => ((width / (max + self.cross_spacing)).ceil().max(1.0)) as usize,
        };
        let usable = (width - self.cross_spacing * (count as f32 - 1.0)).max(0.0);
        let cell = usable / count as f32;
        let ratio = if self.ratio > 0.0 { self.ratio } else { 1.0 };
        Cells { count, width: cell, height: self.extent.unwrap_or(cell / ratio) }
    }

    pub fn height(&self, width: f32, items: usize) -> f32 {
        if items == 0 {
            return 0.0;
        }
        let c = self.cells(width);
        let rows = items.div_ceil(c.count) as f32;
        rows * c.height + (rows - 1.0) * self.main_spacing
    }
}

pub struct Grid {
    spec: GridSpec,
    cells: Vec<Option<El>>,
}

impl Grid {
    pub fn new(spec: GridSpec, cells: Vec<El>) -> Grid {
        Grid { spec, cells: cells.into_iter().map(Some).collect() }
    }
}

impl IntoElement for Grid {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Grid {
    type RequestLayoutState = ();
    type PrepaintState = Vec<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, ()) {
        let (spec, items) = (self.spec, self.cells.len());
        let id = window.request_measured_layout(Style::default(), move |known, available, _, _| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(w) => Some(w),
                _ => None,
            });
            let width = width.unwrap_or(px(0.0));
            size(width, px(spec.height(f32::from(width), items)))
        });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let c = self.spec.cells(f32::from(bounds.size.width));
        let cell = size(AvailableSpace::Definite(px(c.width)), AvailableSpace::Definite(px(c.height)));
        let mut out = Vec::with_capacity(self.cells.len());
        for (i, slot) in self.cells.iter_mut().enumerate() {
            let Some(el) = slot.take() else { continue };
            let mut el = el.w(px(c.width)).h(px(c.height)).flex_none().into_any_element();
            let (row, col) = (i / c.count, i % c.count);
            let at = point(
                px(col as f32 * (c.width + self.spec.cross_spacing)),
                px(row as f32 * (c.height + self.spec.main_spacing)),
            );
            el.layout_as_root(cell, window, cx);
            el.prepaint_at(bounds.origin + at, window, cx);
            out.push(el);
        }
        out
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        cells: &mut Vec<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) {
        for el in cells {
            el.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent(max: f32) -> GridSpec {
        GridSpec { columns: Columns::MaxExtent(max), main_spacing: 10.0, cross_spacing: 10.0, ratio: 1.0, extent: Some(58.0) }
    }

    #[test]
    fn max_extent_columns_follow_flutters_ceil_of_width_over_extent_plus_spacing() {
        assert_eq!(extent(320.0).cells(300.0).count, 1);
        assert_eq!(extent(320.0).cells(331.0).count, 2);
        assert_eq!(extent(320.0).cells(990.0).count, 3);
    }

    #[test]
    fn cells_split_the_width_left_after_spacing() {
        let c = extent(320.0).cells(670.0);
        assert_eq!((c.count, c.width, c.height), (3, 650.0 / 3.0, 58.0));
    }

    #[test]
    fn without_an_extent_the_cell_height_comes_from_the_aspect_ratio() {
        let spec = GridSpec { columns: Columns::Fixed(2), main_spacing: 4.0, cross_spacing: 10.0, ratio: 2.0, extent: None };
        assert_eq!(spec.cells(210.0), Cells { count: 2, width: 100.0, height: 50.0 });
        assert_eq!(spec.height(210.0, 3), 50.0 * 2.0 + 4.0);
    }

    #[test]
    fn an_empty_grid_has_no_height() {
        assert_eq!(extent(320.0).height(500.0, 0), 0.0);
    }
}
