use std::sync::Arc;

use gpui::{
    AnyElement, App, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use mx_theme::{Color, Theme};

use super::dialog::SettingsDialog;
use super::model::SettingsAction;
use crate::chrome::icon::{self, icon};
use crate::chrome::style::hsla;

fn bar(color: Color, w: f32, h: f32) -> Div {
    div().w(px(w)).h(px(h)).rounded(px(h / 2.)).bg(hsla(color))
}

fn dot(color: Color, d: f32) -> Div {
    div().flex_none().size(px(d)).rounded(px(d / 2.)).bg(hsla(color))
}

fn mini(p: &Theme, selected: bool) -> Div {
    let left = div()
        .w(px(52.))
        .h_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(5.))
        .bg(hsla(p.bg_sidebar))
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
    let name = div()
        .flex_1()
        .min_w_0()
        .ml(px(if selected { 3. } else { 5. }))
        .text_size(px(10.5))
        .line_height(px(12.))
        .text_color(hsla(if selected { p.fg } else { p.fg_dim }))
        .font_weight(if selected { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .child(SharedString::from(p.label.clone()));
    let lead = if selected { icon(icon::CHECK, 11., hsla(p.accent)).into_any_element() } else { dot(p.green, 4.).into_any_element() };
    let strip = div()
        .h(px(18.))
        .flex_none()
        .flex()
        .items_center()
        .px(px(4.))
        .bg(hsla(p.bg_sidebar))
        .rounded(px(3.))
        .child(lead)
        .child(name)
        .child(dot(p.red, 4.));
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .size_full()
        .child(div().flex().flex_1().min_h_0().gap(px(3.)).child(left).child(right))
        .child(strip)
}

pub fn swatch(
    p: &Arc<Theme>,
    selected: bool,
    current: &Theme,
    on_pick: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let selector = p.label.clone();
    let ring = if selected { current.accent } else { current.fg_faint.with_alpha(0.) };
    let hover = hsla(current.fg_faint.with_alpha(0.6));
    div()
        .id(SharedString::from(format!("theme-{}", p.id)))
        .debug_selector(move || format!("theme:{selector}"))
        .w(px(176.))
        .h(px(112.))
        .p(px(5.))
        .bg(hsla(p.canvas))
        .rounded(px(9.))
        .border(px(2.))
        .border_color(hsla(ring))
        .cursor_pointer()
        .when_hover(!selected, hover)
        .on_click(move |_, w, cx| on_pick(w, cx))
        .child(mini(p, selected))
        .into_any_element()
}

pub fn gallery(d: &SettingsDialog, current: &Theme) -> Div {
    let data = &d.data;
    let grid = |of: &mut dyn Iterator<Item = &Arc<Theme>>| {
        div().flex().flex_wrap().gap(px(12.)).children(of.map(|p| swatch(p, p.id == data.theme_id, current, d.send(SettingsAction::SetTheme(p.id.clone())))))
    };
    let label = |text: &'static str| div().debug_selector(move || format!("group:{text}")).mb(px(10.)).text_size(px(11.5)).text_color(hsla(current.fg_muted())).child(text);
    let mut col = div()
        .flex()
        .flex_col()
        .child(label("Escuros"))
        .child(grid(&mut data.themes.iter().filter(|p| p.dark)))
        .child(div().h(px(18.)))
        .child(label("Claros"))
        .child(grid(&mut data.themes.iter().filter(|p| !p.dark)));
    if !data.plugin_themes.is_empty() {
        col = col.child(div().h(px(18.))).child(label("De plugins")).child(grid(&mut data.plugin_themes.iter()));
    }
    col
}

trait HoverBorder: Styled + InteractiveElement + Sized {
    fn when_hover(self, on: bool, c: gpui::Hsla) -> Self {
        if on { self.hover(move |s| s.border_color(c)) } else { self }
    }
}

impl<E: Styled + InteractiveElement> HoverBorder for E {}
