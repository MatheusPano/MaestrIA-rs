use gpui::{AnyView, App, AppContext, Context, Hsla, IntoElement, ParentElement, Render, Styled, Window, div, px};
use mx_theme::Palette;

use super::style::{UI_FONT, hsla};

pub struct Tip {
    text: String,
    bg: Hsla,
    border: Hsla,
    fg: Hsla,
}

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().pl(px(10.0)).pt(px(16.0)).child(
            div()
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .bg(self.bg)
                .border_1()
                .border_color(self.border)
                .text_color(self.fg)
                .text_size(px(11.0))
                .font_family(UI_FONT)
                .child(self.text.clone()),
        )
    }
}

pub fn builder(text: String, p: &Palette) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let (bg, border, fg) = (hsla(p.bg_active), hsla(p.border), hsla(p.fg));
    move |_, cx| cx.new(|_| Tip { text: text.clone(), bg, border, fg }).into()
}
