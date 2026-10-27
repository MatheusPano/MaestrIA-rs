use gpui::{
    AnyElement, App, AppContext, Context, DragMoveEvent, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use mx_core::layout::Axis;

use crate::app::host::{StoreHost, update_store};
use crate::app::theme::{GAP, Mx};
use crate::panes::PaneArea;

#[derive(Debug, Clone, PartialEq)]
pub struct GripDrag {
    pub path: Vec<usize>,
    pub gutter: usize,
}

struct Nothing;

impl Render for Nothing {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

pub fn boundary(weights: &[f64], gutter: usize, total: f64) -> f64 {
    let before: f64 = weights.iter().take(gutter + 1).sum();
    before * total + gutter as f64 * GAP as f64
}

pub fn grip(
    mx: &Mx,
    host: &Entity<StoreHost>,
    path: &[usize],
    gutter: usize,
    axis: Axis,
    dragging: bool,
) -> AnyElement {
    let drag = GripDrag {
        path: path.to_vec(),
        gutter,
    };
    let group = SharedString::from(format!("grip:{path:?}:{gutter}"));
    let color = if dragging { mx.accent } else { mx.fg_faint };
    let bar = div().rounded(px(2.)).bg(color);
    let bar = match axis {
        Axis::Row => bar.w(px(3.)).h(px(if dragging { 46. } else { 0. })),
        Axis::Column => bar.h(px(3.)).w(px(if dragging { 46. } else { 0. })),
    };
    let bar = match axis {
        Axis::Row => bar.group_hover(group.clone(), |s| s.h(px(46.))),
        Axis::Column => bar.group_hover(group.clone(), |s| s.w(px(46.))),
    };
    let host_path = path.to_vec();
    let host = host.clone();
    let el = div()
        .id(SharedString::from(format!("grip-el:{path:?}:{gutter}")))
        .group(group)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .on_drag(drag, |_, _, _, cx| cx.new(|_| Nothing))
        .on_click(move |ev, _, cx: &mut App| {
            if ev.click_count() == 2 {
                equalize(&host, &host_path, gutter, cx);
            }
        })
        .child(bar);
    match axis {
        Axis::Row => el.w(px(GAP)).h_full().cursor_col_resize(),
        Axis::Column => el.h(px(GAP)).w_full().cursor_row_resize(),
    }
    .into_any_element()
}

fn equalize(host: &Entity<StoreHost>, path: &[usize], gutter: usize, cx: &mut App) {
    update_store(host, cx, |s| {
        let Some(split) = s.panes_mut().and_then(|p| p.split_at_mut(path)) else {
            return;
        };
        if gutter + 1 >= split.weights.len() {
            return;
        }
        let even = (split.weights[gutter] + split.weights[gutter + 1]) / 2.;
        split.weights[gutter] = even;
        split.weights[gutter + 1] = even;
        s.save();
    });
}

pub fn on_move(host: &Entity<StoreHost>, path: &[usize], axis: Axis, ev: &DragMoveEvent<GripDrag>, cx: &mut Context<PaneArea>) {
    let drag = ev.drag(cx);
    if drag.path != path {
        return;
    }
    let gutter = drag.gutter;
    let b = ev.bounds;
    let (origin, extent, at) = match axis {
        Axis::Row => (b.origin.x, b.size.width, ev.event.position.x),
        Axis::Column => (b.origin.y, b.size.height, ev.event.position.y),
    };
    let extent = f64::from(f32::from(extent));
    let pointer = f64::from(f32::from(at - origin));
    update_store(host, cx, |s| {
        let Some(weights) = s.panes_mut().and_then(|p| p.split_at_mut(path)).map(|split| split.weights.clone()) else {
            return;
        };
        if let Some((delta, total)) = pointer_delta(&weights, gutter, extent, pointer) {
            s.resize_split(path, gutter as isize, delta, total);
        }
    });
}

pub fn pointer_delta(weights: &[f64], gutter: usize, extent: f64, pointer: f64) -> Option<(f64, f64)> {
    let gaps = GAP as f64 * weights.len().saturating_sub(1) as f64;
    let total = extent - gaps;
    if total <= 0. {
        return None;
    }
    let edge = boundary(weights, gutter, total) + GAP as f64 / 2.;
    let delta = pointer - edge;
    (delta.abs() >= 0.5).then_some((delta, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mx_core::layout::{PaneNode, PaneSplit};

    #[test]
    fn the_first_boundary_sits_after_the_first_weight() {
        assert_eq!(boundary(&[0.5, 0.5], 0, 1000.), 500.);
        assert_eq!(boundary(&[0.25, 0.25, 0.5], 1, 1000.), 508.);
    }

    fn three_way() -> PaneSplit {
        let children = ["a", "b", "c"].map(|id| PaneNode::Leaf(id.into())).to_vec();
        PaneSplit { axis: Axis::Row, weights: vec![1. / 3., 1. / 3., 1. / 3.], children }
    }

    fn drag_to(split: &mut PaneSplit, gutter: usize, extent: f64, pointer: f64) {
        if let Some((delta, total)) = pointer_delta(&split.weights, gutter, extent, pointer) {
            split.resize(gutter as isize, delta, total);
        }
    }

    fn edge(split: &PaneSplit, gutter: usize, extent: f64) -> f64 {
        boundary(&split.weights, gutter, extent - 16.) + 4.
    }

    #[test]
    fn a_burst_of_moves_between_frames_lands_the_gutter_under_the_pointer() {
        let mut split = three_way();
        let start = edge(&split, 0, 1500.);
        for step in 1..=10 {
            drag_to(&mut split, 0, 1500., start - 3. * step as f64);
        }
        assert!((edge(&split, 0, 1500.) - (start - 30.)).abs() < 0.5);
    }

    #[test]
    fn repeating_the_same_pointer_position_leaves_the_weights_alone() {
        let mut split = three_way();
        let target = edge(&split, 1, 1500.) + 80.;
        drag_to(&mut split, 1, 1500., target);
        let settled = split.weights.clone();
        for _ in 0..5 {
            drag_to(&mut split, 1, 1500., target);
        }
        assert_eq!(split.weights, settled);
    }

    #[test]
    fn a_steady_drag_never_moves_the_gutter_backwards() {
        let mut split = three_way();
        let mut pointer = edge(&split, 0, 1500.);
        let mut last = pointer;
        for _ in 0..60 {
            pointer -= 2.;
            drag_to(&mut split, 0, 1500., pointer);
            let now = edge(&split, 0, 1500.);
            assert!(now <= last + 1e-9, "gutter went back from {last} to {now}");
            last = now;
        }
    }
}
