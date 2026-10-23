use gpui::{
    AnyElement, App, Div, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::app::icons::{Icon, icon};
use crate::app::theme::{Mx, on_color};
use crate::app::tooltip::tip;

pub fn badge(mx: &Mx, count: usize, color: Option<Hsla>) -> impl IntoElement {
    let bg = color.unwrap_or(mx.red);
    div()
        .flex_none()
        .mr(px(6.))
        .px(px(5.5))
        .py(px(1.5))
        .rounded(px(8.))
        .bg(bg)
        .text_size(px(10.))
        .line_height(px(12.))
        .font_weight(FontWeight::BOLD)
        .text_color(on_color(bg, mx.canvas))
        .child(count.to_string())
}

pub fn count_chip(mx: &Mx, count: usize) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(5.))
        .py(px(1.5))
        .rounded(px(6.))
        .bg(mx.bg_active)
        .text_size(px(10.))
        .line_height(px(12.))
        .text_color(mx.fg_dim)
        .child(count.to_string())
}

pub fn count_text(mx: &Mx, count: usize) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(4.))
        .text_size(px(11.5))
        .text_color(mx.fg_faint)
        .child(count.to_string())
}

pub fn row_button(
    mx: &Mx,
    id: impl Into<ElementId>,
    glyph: Icon,
    tooltip: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .p(px(3.))
        .rounded(px(5.))
        .cursor_pointer()
        .hover(|s| s.bg(mx.bg_hover))
        .tooltip(tip(tooltip.into()))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_click(window, cx)
        })
        .child(icon(glyph, 18., mx.fg_faint))
}

pub fn mini_button(
    mx: &Mx,
    id: impl Into<ElementId>,
    glyph: Icon,
    color: Hsla,
    tooltip: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .size(px(22.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(mx.border))
        .tooltip(tip(tooltip.into()))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_click(window, cx)
        })
        .child(icon(glyph, 14., color))
}

pub fn chevron(mx: &Mx, collapsed: bool, size: f32) -> Div {
    icon(
        if collapsed {
            Icon::ChevronRight
        } else {
            Icon::ExpandMore
        },
        size,
        mx.fg_dim,
    )
}

pub fn nest(rail: f32, line: Hsla, children: Vec<AnyElement>) -> impl IntoElement {
    div()
        .ml(px(rail))
        .border_l_1()
        .border_color(line)
        .flex()
        .flex_col()
        .children(children)
}
