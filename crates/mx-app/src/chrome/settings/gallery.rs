use std::sync::Arc;

use gpui::{
    AnyElement, App, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use mx_theme::{Color, Theme};

use crate::chrome::icon::{self, icon};
use crate::chrome::style::hsla;

fn bar(color: Color, w: f32, h: f32) -> Div {
    div().w(px(w)).h(px(h)).rounded(px(h / 2.)).bg(hsla(color))
}

fn dot(color: Color, d: f32) -> Div {
    div().size(px(d)).rounded(px(d / 2.)).bg(hsla(color))
}

fn mini(p: &Theme) -> Div {
    let left = div()
        .w(px(52.))
        .h_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(5.))
        .bg(hsla(p.bg_sidebar))
        .border(px(0.5))
        .border_color(hsla(p.border))
        .rounded(px(3.))
        .child(bar(p.fg, 26., 3.))
        .child(bar(p.fg_dim, 34., 3.))
        .child(bar(p.accent, 22., 3.))
        .child(bar(p.fg_faint, 30., 3.));
    let right = div()
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(5.))
        .bg(hsla(p.bg))
        .rounded(px(3.))
        .child(div().flex().items_center().gap(px(3.)).child(dot(p.green, 4.)).child(bar(p.ansi.blue, 30., 3.)))
        .child(bar(p.fg, 44., 3.))
        .child(bar(p.ansi.magenta, 24., 3.))
        .child(bar(p.yellow, 34., 3.))
        .child(bar(p.fg_faint, 40., 3.));
    let strip = div()
        .h(px(10.))
        .flex()
        .items_center()
        .gap(px(3.))
        .px(px(4.))
        .bg(hsla(p.bg_sidebar))
        .rounded(px(2.))
        .child(dot(p.green, 3.))
        .child(bar(p.fg_faint, 18., 2.))
        .child(div().flex_1())
        .child(dot(p.red, 3.));
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .size_full()
        .child(div().flex().flex_1().gap(px(4.)).child(left).child(right))
        .child(strip)
}

pub fn swatch(
    p: &Arc<Theme>,
    selected: bool,
    current: &Theme,
    on_pick: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let label: SharedString = p.label.clone().into();
    let selector = label.clone();
    let faint = hsla(current.fg_faint);
    let mut row = div().flex().items_center().gap(px(5.)).pt(px(6.));
    if selected {
        row = row.child(icon(icon::CHECK, 13., hsla(current.accent)));
    }
    let mut text = div().text_size(px(11.5)).text_color(hsla(if selected { current.fg } else { current.fg_dim })).child(label);
    if selected {
        text = text.font_weight(FontWeight::SEMIBOLD);
    }
    row = row.child(text);
    if !p.dark {
        row = row.child(icon(icon::LIGHT_MODE_OUTLINED, 12., hsla(current.fg_faint)));
    }
    let border = if selected { current.accent } else { current.border };
    div()
        .id(SharedString::from(format!("theme-{}", p.id)))
        .debug_selector(move || format!("theme:{selector}"))
        .w(px(176.))
        .cursor_pointer()
        .on_click(move |_, w, cx| on_pick(w, cx))
        .child(
            div()
                .h(px(104.))
                .p(px(6.))
                .bg(hsla(p.canvas))
                .rounded(px(8.))
                .border(px(if selected { 2. } else { 1. }))
                .border_color(hsla(border))
                .when_hover(!selected, faint)
                .child(mini(p)),
        )
        .child(row)
        .into_any_element()
}

trait HoverBorder: Styled + InteractiveElement + Sized {
    fn when_hover(self, on: bool, c: gpui::Hsla) -> Self {
        if on { self.hover(move |s| s.border_color(c)) } else { self }
    }
}

impl<E: Styled + InteractiveElement> HoverBorder for E {}
