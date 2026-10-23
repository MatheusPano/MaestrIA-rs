use gpui::{
    AnyElement, App, Div, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, rgba,
};
use mx_theme::Theme;

use crate::chrome::icon::{self, Icon, icon};
use crate::chrome::style::{Callback, MONO, hsla, shadow, theme};

pub const BARRIER: u32 = 0x0000_008a;

pub fn barrier(color: u32, on_dismiss: Option<Callback>) -> gpui::Stateful<Div> {
    let mut el = div()
        .id("dialog-barrier")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(color))
        .occlude();
    if let Some(f) = on_dismiss {
        el = el.on_click(move |_, window, cx| f(window, cx));
    }
    el
}

pub fn card(t: &Theme, radius: f32) -> gpui::Stateful<Div> {
    div()
        .id("dialog-card")
        .debug_selector(|| "dialog".into())
        .flex()
        .flex_col()
        .bg(hsla(t.bg_sidebar))
        .rounded(px(radius))
        .border_1()
        .border_color(hsla(t.border))
        .shadow(shadow(t, 24., 11.))
        .overflow_hidden()
        .on_click(|_, _, cx| cx.stop_propagation())
}

pub fn alert(
    title: impl Into<SharedString>,
    width: Option<f32>,
    body: impl IntoElement,
    actions: Vec<AnyElement>,
    on_cancel: Callback,
    cx: &App,
) -> impl IntoElement {
    let t = theme(cx);
    let title = title.into();
    let selector = title.clone();
    let mut content = div().px(px(24.)).pt(px(16.)).pb(px(8.)).child(body);
    if let Some(w) = width {
        content = content.w(px(w + 48.));
    }
    barrier(BARRIER, Some(on_cancel)).child(
        card(&t, 12.)
            .min_w(px(280.))
            .max_w(px(900.))
            .child(
                div()
                    .debug_selector(move || format!("title:{selector}"))
                    .px(px(24.))
                    .pt(px(24.))
                    .text_size(px(15.))
                    .text_color(hsla(t.fg))
                    .child(title),
            )
            .child(content)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(24.))
                    .pt(px(8.))
                    .pb(px(24.))
                    .children(actions),
            ),
    )
}

pub fn fact(glyph: Icon, color: Hsla, text: impl Into<SharedString>) -> AnyElement {
    let text = text.into();
    let selector = text.clone();
    div()
        .debug_selector(move || format!("fact:{selector}"))
        .flex()
        .items_center()
        .gap(px(7.))
        .pb(px(6.))
        .child(icon(glyph, 14., color))
        .child(div().flex_1().text_size(px(12.)).text_color(color).child(text))
        .into_any_element()
}

pub fn note(text: impl Into<SharedString>, color: Hsla) -> AnyElement {
    let text = text.into();
    let selector = text.clone();
    div()
        .debug_selector(move || format!("note:{selector}"))
        .text_size(px(11.))
        .line_height(px(15.))
        .text_color(color)
        .child(text)
        .into_any_element()
}

pub fn path_line(path: impl Into<SharedString>, t: &Theme) -> AnyElement {
    div()
        .pb(px(14.))
        .font_family(MONO)
        .text_size(px(11.))
        .text_color(hsla(t.fg_faint))
        .child(path.into())
        .into_any_element()
}

pub fn check_line(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    toggle: impl Fn(&mut Window, &mut App) + 'static,
    t: &Theme,
) -> AnyElement {
    let glyph = if on { icon::CHECK_BOX_ROUNDED } else { icon::CHECK_BOX_OUTLINE_BLANK_ROUNDED };
    let label = label.into();
    let selector = label.clone();
    div()
        .id(id)
        .debug_selector(move || format!("check:{selector}"))
        .mt(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .py(px(6.))
        .px(px(6.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(hsla(t.bg_hover)))
        .on_click(move |_, window, cx| toggle(window, cx))
        .child(icon(glyph, 18., hsla(if on { t.accent } else { t.fg_dim })))
        .child(div().flex_1().text_size(px(12.)).text_color(hsla(t.fg_dim)).child(label))
        .into_any_element()
}
