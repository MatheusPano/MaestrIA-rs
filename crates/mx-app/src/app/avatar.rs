use gpui::{Hsla, IntoElement, ParentElement, Styled, div, px};
use mx_core::model::ClaudeStatus;

use crate::app::icons::{Icon, claude_mark, icon};
use crate::app::theme::Mx;

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
    let glyph = match status {
        ClaudeStatus::Unknown => return div(),
        ClaudeStatus::Working | ClaudeStatus::Tool | ClaudeStatus::Starting => None,
        ClaudeStatus::Ready => Some((Icon::ChevronRightRounded, 0.94)),
        ClaudeStatus::Idle => Some((Icon::Check, 0.8)),
        ClaudeStatus::WaitingInput => Some((Icon::PriorityHigh, 0.8)),
        ClaudeStatus::WaitingAnswer => Some((Icon::QuestionMark, 0.62)),
        ClaudeStatus::WaitingPermission => Some((Icon::Lock, 0.7)),
        ClaudeStatus::Ended => Some((Icon::CloseRounded, 0.8)),
    };
    let disc = div()
        .size(px(size))
        .rounded_full()
        .bg(mx.canvas)
        .border(px(1.2))
        .border_color(color)
        .flex()
        .items_center()
        .justify_center();
    match glyph {
        Some((g, scale)) => disc.child(icon(g, (size * scale).round(), color)),
        None => disc.child(div().size(px(size * 0.4)).rounded_full().bg(color)),
    }
}

pub fn dot(color: Hsla, size: f32) -> impl IntoElement {
    div().flex_none().size(px(size)).rounded_full().bg(color)
}
