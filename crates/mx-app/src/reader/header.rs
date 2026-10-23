use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::*, px,
};
use mx_claude::docs::DocSource;

use super::style::Sheet;

pub type Action = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct HeaderButton {
    pub glyph: &'static str,
    pub tooltip: &'static str,
    pub action: Action,
}

pub fn source_glyph(source: DocSource) -> &'static str {
    match source {
        DocSource::Plan => "☑",
        DocSource::File => "▤",
        DocSource::Message => "✉",
        DocSource::Report => "▦",
        DocSource::Plugin => "⬚",
    }
}

pub fn header(
    sheet: &Sheet,
    glyph: &'static str,
    title: SharedString,
    subtitle: SharedString,
    buttons: Vec<HeaderButton>,
) -> AnyElement {
    let label = div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(8.))
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .child(
            div()
                .flex_shrink()
                .min_w_0()
                .text_size(px(15.))
                .font_weight(FontWeight::BOLD)
                .text_color(sheet.fg)
                .whitespace_nowrap()
                .text_ellipsis()
                .overflow_hidden()
                .child(title),
        )
        .child(
            div()
                .flex_shrink()
                .flex_grow()
                .min_w_0()
                .text_size(px(12.))
                .text_color(sheet.fg_dim)
                .whitespace_nowrap()
                .text_ellipsis()
                .overflow_hidden()
                .child(subtitle),
        );
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .h(px(42.))
        .pl(px(12.))
        .pr(px(8.))
        .bg(sheet.bg_sidebar)
        .border_b_1()
        .border_color(sheet.border)
        .child(
            div()
                .flex_none()
                .w(px(17.))
                .text_size(px(15.))
                .text_color(sheet.fg_dim)
                .child(glyph),
        )
        .child(div().w(px(9.)).flex_none())
        .child(label)
        .child(
            div()
                .flex()
                .flex_row()
                .flex_none()
                .gap(px(2.))
                .children(buttons.into_iter().map(|b| button(sheet, b))),
        )
        .into_any_element()
}

fn button(sheet: &Sheet, b: HeaderButton) -> impl IntoElement {
    let action = b.action;
    let tip = Tip {
        text: b.tooltip.into(),
        sheet: sheet.clone(),
    };
    div()
        .id(b.tooltip)
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .text_size(px(15.))
        .text_color(sheet.fg_dim)
        .cursor_pointer()
        .hover(|s| s.bg(sheet.bg_hover).text_color(sheet.fg))
        .child(b.glyph)
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_click(move |_, window, cx| action(window, cx))
        .tooltip(move |_, cx| cx.new(|_| tip.clone()).into())
}

#[derive(Clone)]
struct Tip {
    text: SharedString,
    sheet: Sheet,
}

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(4.))
            .bg(self.sheet.bg)
            .border_1()
            .border_color(self.sheet.border)
            .rounded(px(4.))
            .shadow_sm()
            .text_size(px(12.))
            .text_color(self.sheet.fg)
            .child(self.text.clone())
    }
}

pub fn notice(sheet: &Sheet, glyph: &'static str, title: &'static str, detail: String) -> AnyElement {
    let mut column = div()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .text_size(px(26.))
                .text_color(sheet.fg_faint)
                .child(glyph),
        )
        .child(
            div()
                .mt(px(12.))
                .text_size(px(13.))
                .text_color(sheet.fg_dim)
                .child(title),
        );
    if !detail.is_empty() {
        column = column.child(
            div()
                .mt(px(6.))
                .w(px(340.))
                .text_center()
                .font_family(super::style::MONO)
                .text_size(px(10.5))
                .text_color(sheet.fg_faint)
                .child(detail),
        );
    }
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .child(column)
        .into_any_element()
}
