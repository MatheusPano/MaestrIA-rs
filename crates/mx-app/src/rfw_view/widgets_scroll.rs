use gpui::{AlignItems, App, Bounds, Pixels, StatefulInteractiveElement, Styled, Window, canvas, prelude::*, px};
use mx_rfw::decoders::{Axis, GridDelegate};
use mx_rfw::widgets::core_layout::SingleChildScrollView;
use mx_rfw::widgets::core_widgets::ScrollList;

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
    if let Some(c) = optional(cx, s.child, item_fit(s.scroll_direction)) {
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
    for c in &l.children {
        let mut item = rigid(child(cx, c.clone(), item_fit(l.scroll_direction)));
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

fn measure(cx: &mut Ctx, id: u64) -> impl IntoElement {
    let view = cx.view.clone();
    canvas(
        move |bounds: Bounds<Pixels>, _: &mut Window, app: &mut App| {
            let width = f32::from(bounds.size.width);
            view.update(app, |v, cx| {
                if v.measured.get(&id).is_none_or(|w| (w - width).abs() > 0.5) {
                    v.measured.insert(id, width);
                    cx.notify();
                }
            })
            .ok();
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .w_full()
    .h(px(1.0))
}

fn grid_view(cx: &mut Ctx, l: &ScrollList, grid: GridDelegate, fit: Fit) -> El {
    let id = cx.next_id();
    let pad = l.padding.map_or(0.0, |p| p.start + p.end);
    let width = cx.measured(id).map(|w| f64::from(w) - pad);
    let (count, main_spacing, cross_spacing, ratio, extent) = match grid {
        GridDelegate::FixedCrossAxisCount {
            cross_axis_count,
            main_axis_spacing,
            cross_axis_spacing,
            child_aspect_ratio,
            main_axis_extent,
        } => (cross_axis_count.max(1) as usize, main_axis_spacing, cross_axis_spacing, child_aspect_ratio, main_axis_extent),
        GridDelegate::MaxCrossAxisExtent {
            max_cross_axis_extent,
            main_axis_spacing,
            cross_axis_spacing,
            child_aspect_ratio,
            main_axis_extent,
        } => {
            let n = width.map_or(1.0, |w| (w / (max_cross_axis_extent + cross_axis_spacing)).ceil());
            (n.max(1.0) as usize, main_axis_spacing, cross_axis_spacing, child_aspect_ratio, main_axis_extent)
        }
    };
    let usable = width.map(|w| (w - cross_spacing * (count as f64 - 1.0)).max(0.0));
    let cell = usable.map(|u| u / count as f64);
    let main = extent.or(cell.map(|c| c / ratio)).unwrap_or(58.0);
    let mut el = if l.shrink_wrap { cx.el().flex().flex_col() } else { scroller(cx, Axis::Vertical, false) };
    el = el.relative().gap(pxf(main_spacing)).child(measure(cx, id));
    if let Some(p) = &l.padding {
        set_padding(el.style(), p);
    }
    let children: Vec<_> = l.children.to_vec();
    for row in children.chunks(count) {
        let mut line = cx.el().flex().flex_row().gap(pxf(cross_spacing)).h(pxf(main)).flex_none();
        for c in row {
            let item = child(cx, c.clone(), Fit::BOTH);
            line = line.child(match cell {
                Some(w) => item.w(pxf(w)).flex_none(),
                None => item.flex_1(),
            });
        }
        el = el.child(line);
    }
    if fit.any() { stretch_self(el) } else { el }
}
