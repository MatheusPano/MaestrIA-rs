use gpui::{Div, Hsla, ParentElement, Styled, div, px};

pub use super::icon_table::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Icon(pub char);

pub const FAMILY: &str = "Material Icons";

pub fn icon(icon: Icon, size: f32, color: impl Into<Hsla>) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(size))
        .font_family(FAMILY)
        .text_size(px(size))
        .line_height(px(size))
        .text_color(color.into())
        .child(icon.0.to_string())
}
