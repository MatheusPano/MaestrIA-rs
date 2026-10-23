use gpui::{FontWeight, IntoElement, ParentElement, Styled, div, px, relative};
use mx_core::layout::{Axis, DropSide};

use crate::app::theme::{Mx, RADIUS};

const CORE: f32 = 0.15;
const MAX_CORE: f32 = 160.;

pub fn side_at(x: f32, y: f32, w: f32, h: f32) -> DropSide {
    let dx = x - w / 2.;
    let dy = y - h / 2.;
    let side = if dx.abs() < (w * CORE).min(MAX_CORE) && dy.abs() < (h * CORE).min(MAX_CORE) {
        DropSide::Center
    } else if (dx / w).abs() >= (dy / h).abs() {
        if dx < 0. { DropSide::Left } else { DropSide::Right }
    } else if dy < 0. {
        DropSide::Top
    } else {
        DropSide::Bottom
    };
    if side == DropSide::Center {
        return side;
    }
    let extent = match side.axis() {
        Axis::Row => w,
        Axis::Column => h,
    };
    if ((extent / 2.) as f64) < side.axis().min_extent() {
        DropSide::Center
    } else {
        side
    }
}

pub fn hint(mx: &Mx, side: DropSide) -> impl IntoElement {
    let half = relative(0.5);
    let full = relative(1.);
    let zero = relative(0.);
    let (left, top, w, h) = match side {
        DropSide::Center => (zero, zero, full, full),
        DropSide::Left => (zero, zero, half, full),
        DropSide::Right => (half, zero, half, full),
        DropSide::Top => (zero, zero, full, half),
        DropSide::Bottom => (zero, half, full, half),
    };
    let label = if side == DropSide::Center { "Trocar" } else { "Dividir" };
    div()
        .absolute()
        .left(left)
        .top(top)
        .w(w)
        .h(h)
        .rounded(px(RADIUS))
        .bg(mx.accent.opacity(0.16))
        .border(px(1.6))
        .border_color(mx.accent.opacity(0.8))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .px(px(10.))
                .py(px(4.))
                .rounded(px(12.))
                .bg(mx.accent)
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(mx.canvas)
                .child(label),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_middle_of_a_pane_swaps() {
        assert_eq!(side_at(500., 300., 1000., 600.), DropSide::Center);
    }

    #[test]
    fn near_an_edge_the_pane_splits_on_that_side() {
        assert_eq!(side_at(20., 300., 1000., 600.), DropSide::Left);
        assert_eq!(side_at(980., 300., 1000., 600.), DropSide::Right);
        assert_eq!(side_at(500., 10., 1000., 600.), DropSide::Top);
        assert_eq!(side_at(500., 590., 1000., 600.), DropSide::Bottom);
    }

    #[test]
    fn the_core_is_capped_at_160_px() {
        assert_eq!(side_at(1000. + 170., 600., 2000., 1200.), DropSide::Right);
        assert_eq!(side_at(1000. + 150., 600., 2000., 1200.), DropSide::Center);
    }

    #[test]
    fn a_side_too_small_to_split_falls_back_to_center() {
        assert_eq!(side_at(10., 150., 400., 300.), DropSide::Center);
        assert_eq!(side_at(250., 5., 500., 250.), DropSide::Center);
    }
}
