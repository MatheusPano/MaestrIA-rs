use gpui::{
    AnyElement, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, px,
};
use mx_core::model::FollowUpKind;
use mx_theme::Theme;

use crate::chrome::icon::{self, Icon, icon};
use crate::chrome::style::hsla;

pub fn step_icon(kind: FollowUpKind) -> Icon {
    match kind {
        FollowUpKind::KeepGoing => icon::SUBDIRECTORY_ARROW_RIGHT,
        FollowUpKind::NewSession => icon::ADD_COMMENT_OUTLINED,
        FollowUpKind::Command => icon::TERMINAL,
        FollowUpKind::Handoff => icon::SWAP_HORIZ,
    }
}

pub fn bullet(child: AnyElement, dim: bool, t: &Theme) -> Div {
    div()
        .size(px(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(12.))
        .bg(hsla(if dim { t.bg } else { t.bg_active }))
        .border_1()
        .border_color(hsla(t.border))
        .child(child)
}

pub fn node(bullet: Div, tail: bool, content: impl IntoElement, t: &Theme) -> AnyElement {
    let mut gutter = div().w(px(34.)).flex_none().flex().flex_col().items_center().child(bullet);
    if tail {
        gutter = gutter.child(div().flex_1().w(px(1.5)).bg(hsla(t.border)));
    }
    div()
        .flex()
        .child(gutter)
        .child(div().flex_1().min_w_0().pb(px(12.)).child(content))
        .into_any_element()
}

pub fn kind_tile(step: usize, kind: FollowUpKind, on: bool, f: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static, t: &Theme) -> AnyElement {
    let short = kind.short();
    div()
        .id(gpui::SharedString::from(format!("kind-{step}-{}", kind.name())))
        .debug_selector(move || format!("kind:{step}:{short}"))
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(9.))
        .py(px(5.))
        .rounded(px(7.))
        .cursor_pointer()
        .bg(hsla(if on { t.bg_active } else { t.bg }))
        .border_1()
        .border_color(hsla(if on { t.accent } else { t.border }))
        .hover(|s| s.bg(hsla(t.bg_hover)))
        .child(icon(step_icon(kind), 14., hsla(if on { t.accent } else { t.fg_dim })))
        .child(div().text_size(px(12.)).text_color(hsla(if on { t.fg } else { t.fg_dim })).child(short))
        .on_click(move |_, w, cx| f(w, cx))
        .into_any_element()
}

pub fn add_tile(kind: FollowUpKind, f: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static, t: &Theme) -> AnyElement {
    let short = kind.short();
    let accent = t.accent;
    div()
        .id(gpui::SharedString::from(format!("add-{}", kind.name())))
        .debug_selector(move || format!("add:{short}"))
        .w(px(202.))
        .flex()
        .gap(px(9.))
        .p(px(10.))
        .rounded(px(9.))
        .cursor_pointer()
        .bg(hsla(t.bg))
        .border_1()
        .border_color(hsla(t.border))
        .hover(move |s| s.border_color(hsla(accent)))
        .child(icon(step_icon(kind), 16., hsla(t.accent)))
        .child(
            div().flex_1().min_w_0().flex().flex_col().gap(px(2.))
                .child(div().text_size(px(12.5)).text_color(hsla(t.fg)).child(short))
                .child(div().text_size(px(11.)).line_height(px(14.)).text_color(hsla(t.fg_faint)).child(kind.blurb())),
        )
        .on_click(move |_, w, cx| f(w, cx))
        .into_any_element()
}
