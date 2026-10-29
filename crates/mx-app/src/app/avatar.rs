use gpui::{
    BorderStyle, Bounds, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Styled, Window, canvas, div,
    point, px, quad, size, transparent_black,
};
use mx_core::model::ClaudeStatus;

use crate::app::icons::{Icon, claude_mark, icon};
use crate::app::theme::Mx;
use crate::chrome::anim::slots::{Animated, Motion};

/// `asleep` is a hibernated session: the moon takes the badge's place, because
/// its status is the `x` of a process that ended and a hibernated one did not.
pub fn claude_avatar(mx: &Mx, status: ClaudeStatus, size: f32, dim: bool, asleep: bool) -> impl IntoElement {
    let badge = (size * 0.56).clamp(9., 14.);
    let offset = -badge * 0.18;
    let opacity = if dim { 0.72 } else { 1. };
    div()
        .flex_none()
        .relative()
        .size(px(size))
        .opacity(opacity)
        .child(claude_mark(size, mx.claude))
        .child(
            div()
                .absolute()
                .right(px(offset))
                .bottom(px(offset))
                .child(if asleep {
                    glyph_disc(mx, mx.fg_dim, badge).child(icon(Icon::Bedtime, (badge * 0.64).round(), mx.fg_dim))
                } else {
                    status_badge(mx, status, badge, opacity)
                }),
        )
}

fn glyph_disc(mx: &Mx, color: Hsla, size: f32) -> gpui::Div {
    div()
        .size(px(size))
        .rounded_full()
        .bg(mx.canvas)
        .border(px(1.2))
        .border_color(color)
        .flex()
        .items_center()
        .justify_center()
}

fn status_badge(mx: &Mx, status: ClaudeStatus, size: f32, opacity: f32) -> gpui::Div {
    let color = mx.status(status);
    let disc = || div().size(px(size)).rounded_full().bg(mx.canvas);
    let (glyph, scale) = match status {
        ClaudeStatus::Unknown => return div(),
        ClaudeStatus::Working | ClaudeStatus::Tool => return disc().child(Animated::new(Motion::Spin, size, mx.theme.accent).opacity(opacity)),
        ClaudeStatus::Starting => return disc().child(Animated::new(Motion::Pulse, size, mx.theme.purple).opacity(opacity)),
        ClaudeStatus::Ready => return glyph_disc(mx, color, size).child(caret(color)),
        ClaudeStatus::Idle => (Icon::Check, 0.8),
        ClaudeStatus::WaitingInput => (Icon::PriorityHigh, 0.8),
        ClaudeStatus::WaitingAnswer => (Icon::QuestionMark, 0.62),
        ClaudeStatus::WaitingPermission => (Icon::Lock, 0.7),
        ClaudeStatus::Ended => (Icon::CloseRounded, 0.8),
    };
    glyph_disc(mx, color, size).child(icon(glyph, (size * scale).round(), color))
}

/// The Ready caret, drawn: the font's chevron sat crooked and low in the disc.
/// Centred by its geometry and nudged right, where an arrow carries its weight.
fn caret(color: Hsla) -> impl IntoElement {
    canvas(|_, _, _| {}, move |b, _, window, _| paint_caret(window, b, color)).size_full()
}

fn caret_points(b: Bounds<Pixels>) -> ([gpui::Point<Pixels>; 3], f32) {
    let s = f32::from(b.size.width.min(b.size.height));
    let c = b.center();
    let (x, y) = (f32::from(c.x) + s * 0.04, f32::from(c.y));
    let (h, w) = (s * 0.2, s * 0.11);
    let stroke = (s * 0.13).clamp(1.3, 2.);
    ([point(px(x - w), px(y - h)), point(px(x + w), px(y)), point(px(x - w), px(y + h))], stroke)
}

fn paint_caret(window: &mut Window, b: Bounds<Pixels>, color: Hsla) {
    let ([top, tip, bottom], stroke) = caret_points(b);
    // Two butt-ended strokes plus a dot on each end and on the joint: round
    // caps and a round join, which lyon's options can't be reached for here.
    let mut path = PathBuilder::stroke(px(stroke));
    path.move_to(top);
    path.line_to(tip);
    path.move_to(tip);
    path.line_to(bottom);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
    for at in [top, tip, bottom] {
        let dot = Bounds::centered_at(at, size(px(stroke), px(stroke)));
        window.paint_quad(quad(dot, px(stroke / 2.), color, px(0.), transparent_black(), BorderStyle::Solid));
    }
}

pub fn dot(color: Hsla, size: f32) -> impl IntoElement {
    div().flex_none().size(px(size)).rounded_full().bg(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_sits_right_of_centre_with_a_clamped_stroke() {
        let b = Bounds::new(point(px(0.), px(0.)), size(px(10.), px(10.)));
        let ([top, tip, bottom], stroke) = caret_points(b);
        assert!((f32::from(tip.x) - 6.5).abs() < 1e-4);
        assert!((f32::from(top.x) - 4.3).abs() < 1e-4 && top.x == bottom.x);
        assert!((f32::from(top.y) - 3.).abs() < 1e-4 && (f32::from(bottom.y) - 7.).abs() < 1e-4);
        assert_eq!(stroke, 1.3);
        let big = Bounds::new(point(px(0.), px(0.)), size(px(40.), px(40.)));
        assert_eq!(caret_points(big).1, 2.);
    }
}
