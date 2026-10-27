use gpui::{Hsla, IntoElement, ParentElement, Styled, div, px};
use mx_core::model::ClaudeStatus;

use crate::app::icons::{Icon, claude_mark, icon};
use crate::app::theme::Mx;
use crate::chrome::anim::slots::{Animated, Motion};

pub fn claude_avatar(mx: &Mx, status: ClaudeStatus, size: f32, dim: bool) -> impl IntoElement {
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
                .child(status_badge(mx, status, badge, opacity)),
        )
}

fn status_badge(mx: &Mx, status: ClaudeStatus, size: f32, opacity: f32) -> impl IntoElement {
    let color = mx.status(status);
    let disc = || div().size(px(size)).rounded_full().bg(mx.canvas);
    let (glyph, scale) = match status {
        ClaudeStatus::Unknown => return div(),
        ClaudeStatus::Working | ClaudeStatus::Tool => return disc().child(Animated::new(Motion::Spin, size, mx.theme.accent).opacity(opacity)),
        ClaudeStatus::Starting => return disc().child(Animated::new(Motion::Pulse, size, mx.theme.purple).opacity(opacity)),
        ClaudeStatus::Ready => (Icon::ChevronRightRounded, 0.94),
        ClaudeStatus::Idle => (Icon::Check, 0.8),
        ClaudeStatus::WaitingInput => (Icon::PriorityHigh, 0.8),
        ClaudeStatus::WaitingAnswer => (Icon::QuestionMark, 0.62),
        ClaudeStatus::WaitingPermission => (Icon::Lock, 0.7),
        ClaudeStatus::Ended => (Icon::CloseRounded, 0.8),
    };
    disc()
        .border(px(1.2))
        .border_color(color)
        .flex()
        .items_center()
        .justify_center()
        .child(icon(glyph, (size * scale).round(), color))
}

pub fn dot(color: Hsla, size: f32) -> impl IntoElement {
    div().flex_none().size(px(size)).rounded_full().bg(color)
}
