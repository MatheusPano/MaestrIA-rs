use gpui::{Hsla, IntoElement, ParentElement, Styled, div, px};
use mx_core::model::ClaudeStatus;

use crate::app::icons::{Icon, claude_mark, icon};
use crate::app::theme::Mx;
use crate::chrome::anim::{Pulse, Spinner};

pub fn claude_avatar(mx: &Mx, status: ClaudeStatus, size: f32, dim: bool) -> impl IntoElement {
    let badge = (size * 0.56).clamp(9., 14.);
    let offset = -badge * 0.18;
    div()
        .flex_none()
        .relative()
        .size(px(size))
        .opacity(if dim { 0.72 } else { 1. })
        .child(claude_mark(size, mx.claude))
        .child(
            div()
                .absolute()
                .right(px(offset))
                .bottom(px(offset))
                .child(status_badge(mx, status, badge)),
        )
}

fn status_badge(mx: &Mx, status: ClaudeStatus, size: f32) -> impl IntoElement {
    let color = mx.status(status);
    let disc = || div().size(px(size)).rounded_full().bg(mx.canvas);
    let (glyph, scale) = match status {
        ClaudeStatus::Unknown => return div(),
        ClaudeStatus::Working | ClaudeStatus::Tool => return disc().child(Spinner::new(size, mx.theme.accent)),
        ClaudeStatus::Starting => return disc().child(Pulse::new(size, mx.theme.purple)),
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
