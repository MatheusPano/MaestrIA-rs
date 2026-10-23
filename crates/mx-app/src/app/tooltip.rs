use gpui::{
    AnyView, App, AppContext, Context, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, px,
};

use crate::app::theme::Mx;
use crate::chrome::widgets::tip_gate::gated;

pub struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx);
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(mx.bg_active)
            .border_1()
            .border_color(mx.border)
            .text_size(px(11.))
            .text_color(mx.fg)
            .child(self.0.clone())
    }
}

pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    gated(move |_, cx| cx.new(|_| Tip(text.clone())).into())
}
