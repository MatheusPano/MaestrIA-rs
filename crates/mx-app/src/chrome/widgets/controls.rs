use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::chrome::icon;
use crate::chrome::style::{Callback, MONO, hsla, theme};
use crate::chrome::widgets::IconButton;

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    on: bool,
    on_toggle: Callback,
}

impl Switch {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        on: bool,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            on,
            on_toggle: Rc::new(on_toggle),
        }
    }

    pub fn subtitle(mut self, text: impl Into<SharedString>) -> Self {
        self.subtitle = Some(text.into());
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let track = if self.on { t.accent } else { t.bg_active };
        let knob = if self.on { t.canvas } else { t.fg_dim };
        let toggle = self.on_toggle.clone();
        let title = self.title.clone();
        let mut text = div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(hsla(t.fg))
                    .child(self.title),
            );
        if let Some(sub) = self.subtitle {
            text = text.child(
                div()
                    .text_size(px(11.5))
                    .line_height(px(15.5))
                    .text_color(hsla(t.fg_faint))
                    .child(sub),
            );
        }
        div()
            .id(self.id)
            .debug_selector(move || format!("switch:{title}"))
            .flex()
            .items_center()
            .gap(px(14.))
            .py(px(6.))
            .cursor_pointer()
            .on_click(move |_, window, cx| toggle(window, cx))
            .child(text)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .w(px(36.))
                    .h(px(20.))
                    .px(px(3.))
                    .rounded(px(10.))
                    .bg(hsla(track))
                    .border_1()
                    .border_color(hsla(if self.on { t.accent } else { t.border }))
                    .when_on(self.on)
                    .child(div().size(px(14.)).rounded(px(7.)).bg(hsla(knob))),
            )
    }
}

trait WhenOn {
    fn when_on(self, on: bool) -> Self;
}

impl WhenOn for gpui::Div {
    fn when_on(self, on: bool) -> Self {
        if on { self.justify_end() } else { self }
    }
}

#[derive(IntoElement)]
pub struct Stepper {
    id: SharedString,
    label: SharedString,
    value: SharedString,
    on_less: Option<Callback>,
    on_more: Option<Callback>,
}

impl Stepper {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        on_less: Option<Callback>,
        on_more: Option<Callback>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value: value.into(),
            on_less,
            on_more,
        }
    }
}

impl RenderOnce for Stepper {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let button = |suffix: &str, glyph, tooltip: &'static str, f: Option<Callback>| {
            let mut b = IconButton::new(SharedString::from(format!("{}-{suffix}", self.id)), glyph)
                .size(28., 15., 6.)
                .color(t.fg)
                .tooltip(tooltip)
                .selector(format!("step:{}-{suffix}", self.id))
                .disabled(f.is_none());
            if let Some(f) = f {
                b = b.on_click(move |w, cx| f(w, cx));
            }
            b
        };
        let value = self.value.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(hsla(t.fg_faint))
                    .child(self.label.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .p(px(2.))
                    .bg(hsla(t.bg))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(hsla(t.border))
                    .child(button("less", icon::REMOVE, "Menos", self.on_less))
                    .child(
                        div()
                            .id(SharedString::from(format!("{}-value", self.id)))
                            .debug_selector(move || format!("value:{value}"))
                            .w(px(58.))
                            .flex()
                            .justify_center()
                            .font_family(MONO)
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(hsla(t.fg))
                            .child(self.value),
                    )
                    .child(button("more", icon::ADD, "Mais", self.on_more)),
            )
    }
}
