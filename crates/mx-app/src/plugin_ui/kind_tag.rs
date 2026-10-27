use gpui::{Div, ParentElement, Styled, div, px};
use mx_plugins::catalog::PluginKind;
use mx_theme::Theme;

use crate::chrome::icon::{self, icon};
use crate::chrome::style::{alpha, hsla};

#[allow(dead_code)]
pub fn kind_tag(kind: PluginKind, t: &Theme) -> Div {
    let (glyph, color) = match kind {
        PluginKind::Theme => (icon::PALETTE_OUTLINED, t.accent),
        PluginKind::Feature => (icon::EXTENSION_OUTLINED, t.fg_dim),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .px(px(6.))
        .py(px(1.5))
        .rounded(px(4.))
        .bg(alpha(color, 0.12))
        .child(icon(glyph, 11., hsla(color)))
        .child(div().text_size(px(10.5)).text_color(hsla(color)).child(kind.label()))
}
