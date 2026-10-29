use std::rc::Rc;

use gpui::{
    App, ElementId, Hsla, Rgba, FontWeight, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use mx_theme::Color;

use crate::chrome::icon;
use crate::chrome::style::{Callback, MONO, hsla, on as on_color, shadow, theme};
use crate::chrome::widgets::IconButton;

pub type ChangeHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct MxSwitch {
    id: ElementId,
    value: bool,
    on_change: Option<ChangeHandler>,
}

impl MxSwitch {
    pub const WIDTH: f32 = 30.;
    pub const HEIGHT: f32 = 18.;
    const INSET: f32 = 2.;

    pub fn new(id: impl Into<ElementId>, value: bool) -> Self {
        Self { id: id.into(), value, on_change: None }
    }

    pub fn on_change(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    pub fn handler(mut self, f: Option<ChangeHandler>) -> Self {
        self.on_change = f;
        self
    }
}

pub fn lit_track(accent: Color) -> Hsla {
    let mix = |c: f64| (c + (1. - c) * 0.08) as f32;
    Rgba { r: mix(accent.red()), g: mix(accent.green()), b: mix(accent.blue()), a: accent.alpha() as f32 }.into()
}

impl RenderOnce for MxSwitch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let on = self.value;
        let knob_on = on_color(t.accent, t.canvas);
        let (track, hover_track) = if on { (hsla(t.accent), lit_track(t.accent)) } else { (hsla(t.bg_active), hsla(t.bg_hover)) };
        let knob = Self::HEIGHT - 2. * Self::INSET;
        let mut knob_el = div().size(px(knob)).rounded(px(knob / 2.)).bg(if on { knob_on } else { hsla(t.fg_dim) });
        if on {
            knob_el = knob_el.shadow(shadow(&t, 2., 1.));
        }
        let border = if on { track } else { hsla(t.border) };
        let hover_border = if on { hover_track } else { hsla(t.border) };
        let value = on;
        let mut track_el = div()
            .id(self.id)
            .debug_selector(|| "mx-switch".to_owned())
            .flex()
            .flex_none()
            .items_center()
            .w(px(Self::WIDTH))
            .h(px(Self::HEIGHT))
            .p(px(Self::INSET - 1.))
            .rounded(px(Self::HEIGHT / 2.))
            .bg(track)
            .border_1()
            .border_color(border)
            .when(on, |d| d.justify_end())
            .child(knob_el);
        if let Some(f) = self.on_change {
            track_el = track_el
                .cursor_pointer()
                .hover(move |s| s.bg(hover_track).border_color(hover_border))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    f(!value, window, cx)
                });
        } else {
            track_el = track_el.opacity(0.45);
        }
        track_el
    }
}

#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    on: bool,
    on_toggle: Option<Callback>,
}

impl Toggle {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        on: bool,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self { id: id.into(), title: title.into(), subtitle: None, on, on_toggle: Some(Rc::new(on_toggle)) }
    }

    pub fn subtitle(mut self, text: impl Into<SharedString>) -> Self {
        self.subtitle = Some(text.into());
        self
    }

    pub fn disabled(mut self, off: bool) -> Self {
        if off {
            self.on_toggle = None;
        }
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let title = self.title.clone();
        let has_sub = self.subtitle.is_some();
        let mut text = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(2.))
            .child(div().text_size(px(12.5)).text_color(hsla(t.fg)).child(self.title));
        if let Some(sub) = self.subtitle {
            text = text.child(div().text_size(px(11.5)).line_height(px(16.1)).text_color(hsla(t.fg_faint)).child(sub));
        }
        let switch_id = ElementId::Name(SharedString::from(format!("{:?}-switch", self.id)));
        let handler: Option<ChangeHandler> =
            self.on_toggle.clone().map(|f| Rc::new(move |_: bool, w: &mut Window, cx: &mut App| f(w, cx)) as ChangeHandler);
        let toggle = self.on_toggle;
        div()
            .id(self.id)
            .debug_selector(move || format!("switch:{title}"))
            .flex()
            .gap(px(24.))
            .py(px(6.))
            .when(has_sub, |d| d.items_start())
            .when(!has_sub, |d| d.items_center())
            .when_some(toggle, |d, f| d.cursor_pointer().on_click(move |_, window, cx| f(window, cx)))
            .child(text)
            .child(div().flex_none().when(has_sub, |d| d.pt(px(1.))).child(MxSwitch::new(switch_id, self.on).handler(handler)))
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

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Context, IntoElement, Modifiers, ParentElement, Render, Styled, TestAppContext, Window, div};

    use super::MxSwitch;

    struct Host {
        value: bool,
        enabled: bool,
        got: Rc<RefCell<Vec<bool>>>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let got = self.got.clone();
            let s = MxSwitch::new("s", self.value);
            let s = if self.enabled { s.on_change(move |v, _, _| got.borrow_mut().push(v)) } else { s };
            div().size_full().p_4().child(s)
        }
    }

    fn mount(cx: &mut TestAppContext, value: bool, enabled: bool) -> (Rc<RefCell<Vec<bool>>>, &mut gpui::VisualTestContext) {
        let got: Rc<RefCell<Vec<bool>>> = Rc::default();
        let g = got.clone();
        let (_, vcx) = cx.add_window_view(move |_, _| Host { value, enabled, got: g });
        vcx.run_until_parked();
        (got, vcx)
    }

    #[gpui::test]
    fn tapping_a_switch_that_is_off_reports_true(cx: &mut TestAppContext) {
        let (got, vcx) = mount(cx, false, true);
        let b = vcx.debug_bounds("mx-switch").expect("switch drawn");
        vcx.simulate_click(b.center(), Modifiers::none());
        assert_eq!(got.borrow().as_slice(), &[true]);
    }

    #[gpui::test]
    fn a_switch_without_a_handler_ignores_taps(cx: &mut TestAppContext) {
        let (got, vcx) = mount(cx, true, false);
        let b = vcx.debug_bounds("mx-switch").expect("switch drawn");
        vcx.simulate_click(b.center(), Modifiers::none());
        vcx.run_until_parked();
        assert!(got.borrow().is_empty());
    }

    #[gpui::test]
    fn the_switch_is_30_by_18(cx: &mut TestAppContext) {
        let (_, vcx) = mount(cx, true, true);
        let b = vcx.debug_bounds("mx-switch").expect("switch drawn");
        assert_eq!((f32::from(b.size.width), f32::from(b.size.height)), (MxSwitch::WIDTH, MxSwitch::HEIGHT));
    }
}
