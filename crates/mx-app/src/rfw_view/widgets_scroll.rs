use gpui::{AlignItems, FlexDirection, StatefulInteractiveElement, Styled, prelude::*, px, relative};
use mx_rfw::decoders::{Axis, GridDelegate};
use mx_rfw::widgets::core_layout::SingleChildScrollView;
use mx_rfw::widgets::core_widgets::ScrollList;

use super::grid::{Columns, Grid, GridSpec};
use super::layout::{Ctx, El, Fit, rigid, stretch_self};
use super::style::{pxf, set_padding};
use super::widgets_core::{child, optional};

fn scroller(cx: &mut Ctx, axis: Axis, reverse: bool) -> El {
    let el = cx.el().flex().min_w(px(0.0)).min_h(px(0.0));
    let mut el = match (axis, reverse) {
        (Axis::Vertical, false) => el.flex_col().overflow_y_scroll(),
        (Axis::Vertical, true) => el.flex_col_reverse().overflow_y_scroll(),
        (Axis::Horizontal, false) => el.flex_row().overflow_x_scroll(),
        (Axis::Horizontal, true) => el.flex_row_reverse().overflow_x_scroll(),
    };
    el.style().align_items = Some(AlignItems::Stretch);
    el
}

fn item_fit(axis: Axis) -> Fit {
    match axis {
        Axis::Vertical => Fit { w: true, h: false },
        Axis::Horizontal => Fit { w: false, h: true },
    }
}

pub fn single(cx: &mut Ctx, s: &SingleChildScrollView, fit: Fit) -> El {
    let mut el = scroller(cx, s.scroll_direction, s.reverse);
    if let Some(p) = &s.padding {
        set_padding(el.style(), p);
    }
    let horizontal = s.scroll_direction == Axis::Horizontal;
    if let Some(mut c) = cx.within(!horizontal, |cx| optional(cx, s.child, item_fit(s.scroll_direction))) {
        if horizontal && fills_cross(&mut c) {
            el = el.h(relative(1.0));
        }
        el = el.child(rigid(c));
    }
    if fit.any() { stretch_self(el) } else { el }
}

pub fn list(cx: &mut Ctx, l: &ScrollList, fit: Fit) -> El {
    if let Some(grid) = l.grid_delegate {
        return grid_view(cx, l, grid, fit);
    }
    let mut el = scroller(cx, l.scroll_direction, l.reverse);
    if let Some(p) = &l.padding {
        set_padding(el.style(), p);
    }
    let horizontal = l.scroll_direction == Axis::Horizontal;
    for c in &l.children {
        let mut item = rigid(cx.within(!horizontal, |cx| child(cx, c.clone(), item_fit(l.scroll_direction))));
        if let Some(extent) = l.item_extent {
            item = match l.scroll_direction {
                Axis::Vertical => item.h(pxf(extent)),
                Axis::Horizontal => item.w(pxf(extent)),
            };
        }
        el = el.child(item);
    }
    if !l.shrink_wrap && fit.any() { stretch_self(el) } else { el }
}

fn grid_view(cx: &mut Ctx, l: &ScrollList, grid: GridDelegate, fit: Fit) -> El {
    let spec = match grid {
        GridDelegate::FixedCrossAxisCount {
            cross_axis_count,
            main_axis_spacing,
            cross_axis_spacing,
            child_aspect_ratio,
            main_axis_extent,
        } => GridSpec {
            columns: Columns::Fixed(cross_axis_count.max(1) as usize),
            main_spacing: main_axis_spacing as f32,
            cross_spacing: cross_axis_spacing as f32,
            ratio: child_aspect_ratio as f32,
            extent: main_axis_extent.map(|e| e as f32),
        },
        GridDelegate::MaxCrossAxisExtent {
            max_cross_axis_extent,
            main_axis_spacing,
            cross_axis_spacing,
            child_aspect_ratio,
            main_axis_extent,
        } => GridSpec {
            columns: Columns::MaxExtent(max_cross_axis_extent as f32),
            main_spacing: main_axis_spacing as f32,
            cross_spacing: cross_axis_spacing as f32,
            ratio: child_aspect_ratio as f32,
            extent: main_axis_extent.map(|e| e as f32),
        },
    };
    let mut el = if l.shrink_wrap { cx.el().flex().flex_col() } else { scroller(cx, Axis::Vertical, false) };
    if let Some(p) = &l.padding {
        set_padding(el.style(), p);
    }
    let cells = cx.within(true, |cx| l.children.iter().map(|c| child(cx, c.clone(), Fit::BOTH)).collect());
    el = el.child(Grid::new(spec, cells));
    if fit.any() { stretch_self(el) } else { el }
}

fn fills_cross(c: &mut El) -> bool {
    let s = c.style();
    s.flex_direction == Some(FlexDirection::Row) && s.align_items == Some(AlignItems::Stretch)
}
