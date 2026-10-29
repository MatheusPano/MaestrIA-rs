use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Div, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::app::icons::{Icon, icon};
use crate::app::menus::{self, Target};
use crate::app::theme::{Mx, on_color};
use crate::app::tooltip::tip;
use crate::chrome::anim::ease::ease_out;
use crate::chrome::notices::anim_off;

pub const REVEAL: Duration = Duration::from_millis(110);

/// Which header row has the pointer, and since when: the row buttons (the
/// ···, the +, the clear) fade in and out with it instead of popping.
#[derive(Default)]
pub struct Hover {
    on: Option<(SharedString, Instant)>,
    off: Option<(SharedString, Instant)>,
}

impl Hover {
    pub fn set(&mut self, key: SharedString, on: bool, now: Instant) -> bool {
        let current = self.on.as_ref().is_some_and(|(k, _)| *k == key);
        if on == current {
            return false;
        }
        if on {
            if let Some((left, _)) = self.on.take() {
                self.off = Some((left, now));
            }
            self.on = Some((key, now));
        } else {
            self.on = None;
            self.off = Some((key, now));
        }
        true
    }

    pub fn opacity(&self, key: &str, now: Instant) -> f32 {
        let t = |since: &Instant| {
            if anim_off() {
                1.
            } else {
                (now.saturating_duration_since(*since).as_secs_f32() / REVEAL.as_secs_f32()).min(1.)
            }
        };
        match (&self.on, &self.off) {
            (Some((k, since)), _) if k == key => ease_out(t(since)),
            (_, Some((k, since))) if k == key => 1. - ease_out(t(since)),
            _ => 0.,
        }
    }

    pub fn moving(&self, now: Instant) -> bool {
        !anim_off()
            && [&self.on, &self.off]
                .into_iter()
                .flatten()
                .any(|(_, since)| now.saturating_duration_since(*since) < REVEAL)
    }
}

/// A row control that only shows with the pointer on its row. Hidden, it
/// keeps its place so the row doesn't shift, and invisible is unclickable.
pub fn reveal(opacity: f32, child: impl IntoElement) -> Div {
    div()
        .flex_none()
        .opacity(opacity)
        .when(opacity <= 0., |d| d.invisible())
        .child(child)
}

pub fn badge(mx: &Mx, count: usize, color: Option<Hsla>) -> impl IntoElement {
    let bg = color.unwrap_or(mx.red);
    div()
        .flex_none()
        .mr(px(6.))
        .px(px(5.5))
        .py(px(1.5))
        .rounded(px(8.))
        .bg(bg)
        .text_size(px(10.))
        .line_height(px(12.))
        .font_weight(FontWeight::BOLD)
        .text_color(on_color(bg, mx.canvas))
        .child(count.to_string())
}

pub fn count_chip(mx: &Mx, count: usize) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(5.))
        .py(px(1.5))
        .rounded(px(6.))
        .bg(mx.bg_active)
        .text_size(px(10.))
        .line_height(px(12.))
        .text_color(mx.fg_dim)
        .child(count.to_string())
}

pub fn count_text(mx: &Mx, count: usize) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(4.))
        .text_size(px(11.))
        .text_color(mx.fg_muted)
        .child(count.to_string())
}

pub fn row_button(
    mx: &Mx,
    id: impl Into<ElementId>,
    glyph: Icon,
    tooltip: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .p(px(3.))
        .rounded(px(5.))
        .cursor_pointer()
        .hover(|s| s.bg(mx.bg_hover))
        .tooltip(tip(tooltip.into()))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_click(window, cx)
        })
        .child(icon(glyph, 18., mx.fg_faint))
}

pub fn mini_button(
    mx: &Mx,
    id: impl Into<ElementId>,
    glyph: Icon,
    color: Hsla,
    tooltip: impl Into<SharedString>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .size(px(22.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(mx.border))
        .tooltip(tip(tooltip.into()))
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_click(window, cx)
        })
        .child(icon(glyph, 14., color))
}

pub fn chevron(mx: &Mx, collapsed: bool, size: f32) -> Div {
    icon(
        if collapsed {
            Icon::ChevronRight
        } else {
            Icon::ExpandMore
        },
        size,
        mx.fg_dim,
    )
}

pub fn nest(rail: f32, line: Hsla, children: Vec<AnyElement>) -> impl IntoElement {
    div()
        .ml(px(rail))
        .border_l_1()
        .border_color(line)
        .flex()
        .flex_col()
        .children(children)
}

pub fn menu_on(target: Target) -> impl Fn(&mut Window, &mut App) + 'static {
    move |window, cx| menus::open(target.clone(), window, cx)
}

pub fn right_click(target: Target) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static {
    move |ev, _, cx| {
        cx.stop_propagation();
        menus::open_at(target.clone(), ev.position, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_buttons_fade_in_with_the_pointer_and_out_after_it() {
        let t0 = Instant::now();
        let mut h = Hover::default();
        assert_eq!(h.opacity("a", t0), 0.);
        assert!(h.set("a".into(), true, t0));
        assert!(!h.set("a".into(), true, t0));
        assert_eq!(h.opacity("a", t0), 0.);
        assert!(h.moving(t0));
        assert_eq!(h.opacity("a", t0 + REVEAL), 1.);
        assert!(!h.moving(t0 + REVEAL));
        let t1 = t0 + REVEAL * 2;
        assert!(h.set("b".into(), true, t1));
        assert_eq!(h.opacity("a", t1), 1.);
        assert_eq!(h.opacity("a", t1 + REVEAL), 0.);
        assert!(!h.set("a".into(), false, t1 + REVEAL));
        assert!(h.set("b".into(), false, t1 + REVEAL));
        assert_eq!(h.opacity("b", t1 + REVEAL * 2), 0.);
    }
}
