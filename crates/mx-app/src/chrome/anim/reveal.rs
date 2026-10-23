use gpui::{Div, Styled, div, px};
use mx_theme::Theme;

use crate::chrome::style::{alpha, hsla};

pub const REVEAL_ALIGNMENT: f32 = 0.3;

pub fn reveal_overlay(t: &Theme) -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(px(6.))
        .bg(alpha(t.accent, 0.12))
        .border(px(1.5))
        .border_color(hsla(t.accent))
}

pub fn reveal_offset(row_top: f32, row_height: f32, viewport: f32, content: f32) -> f32 {
    let target = row_top - REVEAL_ALIGNMENT * (viewport - row_height);
    target.clamp(0., (content - viewport).max(0.))
}
