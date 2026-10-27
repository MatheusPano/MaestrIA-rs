use std::time::Duration;

use gpui::{
    Animation, AnimationExt, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, px,
};

use super::anim_off;
use crate::chrome::anim::ease::ease_out;
use crate::chrome::icon::{self, icon};
use crate::chrome::style::{Callback, alpha, hsla, shadow, theme};
use crate::chrome::widgets::IconButton;

pub const ENTER: Duration = Duration::from_millis(160);

#[derive(IntoElement)]
pub struct Banner {
    text: SharedString,
    on_close: Callback,
    from_top: bool,
}

impl Banner {
    pub fn new(text: impl Into<SharedString>, on_close: Callback) -> Self {
        Self { text: text.into(), on_close, from_top: false }
    }

    pub fn on_top(mut self, top: bool) -> Self {
        self.from_top = top;
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let close = self.on_close.clone();
        let selector = self.text.clone();
        let card = div()
            .debug_selector(move || format!("banner:{selector}"))
            .relative()
            .max_w(px(460.))
            .flex()
            .items_start()
            .bg(hsla(t.bg_sidebar))
            .rounded(px(8.))
            .border_1()
            .border_color(alpha(t.accent, 0.7))
            .shadow(shadow(&t, 18., 6.))
            .pl(px(14.))
            .pt(px(12.))
            .pr(px(8.))
            .pb(px(12.))
            .child(div().pt(px(1.)).child(icon(icon::INFO_OUTLINE, 16., hsla(t.accent))))
            .child(
                div()
                    .flex_1()
                    .mx(px(10.))
                    .text_size(px(12.5))
                    .line_height(px(17.5))
                    .text_color(hsla(t.fg))
                    .child(self.text.clone()),
            )
            .child(
                IconButton::new("banner-close", icon::CLOSE)
                    .size(20., 14., 4.)
                    .on_click(move |w, cx| close(w, cx)),
            );
        let sign = if self.from_top { -1. } else { 1. };
        let ms = if anim_off() { 1 } else { ENTER.as_millis() as u64 };
        card.with_animation(
            ElementId::Name(format!("banner-{}", self.text).into()),
            Animation::new(Duration::from_millis(ms)).with_easing(ease_out),
            move |el, d| el.opacity(d).top(px(sign * 8. * (1. - d))),
        )
    }
}
