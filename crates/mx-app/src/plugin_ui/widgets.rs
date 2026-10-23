use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, rgba,
};
use mx_store::DialogAnswer;
use mx_theme::Theme;

use crate::app::host::{StoreHost, update_store};
use crate::chrome::style::hsla;

pub type Close = Rc<dyn Fn(&mut Window, &mut App)>;

pub const PICK_BARRIER: u32 = 0x0000_0033;

pub fn answer(host: &Entity<StoreHost>, a: DialogAnswer, cx: &mut App) {
    update_store(host, cx, |s| s.answer_dialog(a));
}

pub fn barrier(color: u32, dismiss: impl Fn(&mut Window, &mut App) + 'static) -> gpui::Stateful<Div> {
    div()
        .id("plugin-barrier")
        .absolute()
        .inset_0()
        .bg(rgba(color))
        .occlude()
        .on_click(move |_, window, cx| dismiss(window, cx))
}

pub fn card(t: &Theme, width: f32) -> gpui::Stateful<Div> {
    div()
        .id("plugin-dialog")
        .w(px(width))
        .flex()
        .flex_col()
        .bg(hsla(t.bg_sidebar))
        .rounded(px(12.))
        .border_1()
        .border_color(hsla(t.border))
        .shadow(crate::chrome::style::shadow(t, 24., 11.))
        .overflow_hidden()
        .on_click(|_, _, cx| cx.stop_propagation())
}

pub fn title(t: &Theme, text: impl Into<SharedString>) -> Div {
    div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(text.into())
}

pub fn dim(t: &Theme, text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(hsla(t.fg_dim)).child(text.into())
}

pub fn text_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    color: Hsla,
    filled: Option<Hsla>,
    act: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut b = div()
        .id(gpui::ElementId::Name(id.into()))
        .px(px(12.))
        .py(px(6.))
        .rounded(px(7.))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .text_color(color)
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            act(window, cx)
        });
    b = match filled {
        Some(bg) => b.bg(bg).hover(|s| s.opacity(0.9)),
        None => b.hover(|s| s.bg(color.opacity(0.1))),
    };
    b.child(label.into()).into_any_element()
}

pub fn chip(id: String, label: String, on: bool, t: &Theme, act: impl Fn(&mut Window, &mut App) + 'static) -> AnyElement {
    let (fg, bg) = if on { (hsla(t.on_accent()), hsla(t.accent)) } else { (hsla(t.fg_dim), hsla(t.bg_hover)) };
    div()
        .id(gpui::ElementId::Name(id.into()))
        .px(px(10.))
        .py(px(4.))
        .rounded(px(12.))
        .bg(bg)
        .text_color(fg)
        .text_size(px(12.))
        .cursor_pointer()
        .on_click(move |_, window, cx| act(window, cx))
        .child(label)
        .into_any_element()
}
