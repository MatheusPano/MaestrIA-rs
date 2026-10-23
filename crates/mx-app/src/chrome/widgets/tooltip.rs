use gpui::{
    AnyView, App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, px,
};

use crate::chrome::style::{hsla, theme};

pub struct Tip {
    text: SharedString,
    max_width: Option<f32>,
}

impl Render for Tip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let card = div()
            .bg(hsla(t.bg_active))
            .border_1()
            .border_color(hsla(t.border))
            .rounded(px(6.))
            .px(px(8.))
            .py(px(4.))
            .text_size(px(11.))
            .text_color(hsla(t.fg))
            .child(self.text.clone());
        match self.max_width {
            Some(w) => card.max_w(px(w)),
            None => card,
        }
    }
}

pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| {
        cx.new(|_| Tip {
            text: text.clone(),
            max_width: None,
        })
        .into()
    }
}

pub fn wide_tip(
    text: impl Into<SharedString>,
    max_width: f32,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| {
        cx.new(|_| Tip {
            text: text.clone(),
            max_width: Some(max_width),
        })
        .into()
    }
}
