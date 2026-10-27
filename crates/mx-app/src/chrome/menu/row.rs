use gpui::{
    AnyElement, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    Stateful, Styled, div, px, svg,
};
use mx_theme::{CLAUDE, Theme};

use super::model::{Glyph, MenuItem, Trailing};
use crate::chrome::assets::CLAUDE_SVG;
use crate::chrome::icon::{self, icon};
use crate::chrome::style::{MONO, alpha, hsla};
use crate::chrome::widgets::MxHover;

const ROW_PAD: f32 = 14.;
const ROW_INSET: f32 = 5.;

pub fn glyph(g: Glyph, t: &Theme, faded: bool) -> AnyElement {
    let el = match g {
        Glyph::Claude => claude_mark(13.),
        Glyph::Icon(i, tone, size) => icon(i, size, hsla(tone.color(t))).into_any_element(),
    };
    let slot = div().w(px(16.)).flex().flex_none().justify_center().child(el);
    if faded {
        slot.opacity(0.45).into_any_element()
    } else {
        slot.into_any_element()
    }
}

pub fn item_row<T>(id: usize, item: &MenuItem<T>, t: &Theme, submenu: bool, open: bool) -> Stateful<Div> {
    let label_color = if item.disabled {
        t.fg_faint
    } else {
        item.tone.map_or(t.fg, |tone| tone.color(t))
    };
    let mut text = div().flex().flex_col().flex_1().min_w_0().child(
        div()
            .text_size(px(12.))
            .line_height(px(14.4))
            .text_color(hsla(label_color))
            .whitespace_nowrap()
            .overflow_hidden()
            .text_ellipsis()
            .child(item.label.clone()),
    );
    if let Some((sub, tone)) = &item.subtitle {
        text = text.child(
            div()
                .mt(px(2.))
                .font_family(MONO)
                .text_size(px(10.5))
                .text_color(hsla(tone.color(t)))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(sub.clone()),
        );
    }
    let label = item.label.clone();
    let mut row = div()
        .id(("menu-row", id))
        .debug_selector(move || format!("menu:{label}"))
        .flex()
        .items_center()
        .h(px(item.height()))
        .pl(px(ROW_PAD - ROW_INSET + item.indent))
        .pr(px(ROW_PAD - ROW_INSET))
        .gap(px(9.))
        .inset_hover(ROW_INSET, 0., 6., (!item.disabled).then(|| hsla(t.bg_hover)));
    if open {
        row = row.bg(hsla(t.bg_hover));
    }
    if !item.disabled {
        row = row.cursor_pointer();
    }
    row = row.child(match item.glyph {
        Some(g) => glyph(g, t, item.disabled),
        None => div().w(px(16.)).flex_none().into_any_element(),
    });
    row = row.child(text);
    if let Some(chord) = &item.chord {
        row = row.child(
            div()
                .ml(px(1.))
                .flex_none()
                .text_size(px(11.))
                .text_color(hsla(t.fg_faint))
                .child(chord.clone()),
        );
    }
    for tr in &item.trailing {
        row = row.child(trailing(tr, t));
    }
    if submenu {
        row = row.child(icon(icon::CHEVRON_RIGHT, 14., hsla(t.fg_faint)));
    }
    row
}

fn trailing(tr: &Trailing, t: &Theme) -> AnyElement {
    match tr {
        Trailing::Chip(text) => div()
            .flex_none()
            .px(px(5.))
            .py(px(1.5))
            .rounded(px(6.))
            .bg(hsla(t.bg_hover))
            .text_size(px(10.))
            .text_color(hsla(t.fg_dim))
            .child(text.clone())
            .into_any_element(),
        Trailing::Ghost(n) => div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .px(px(5.))
            .py(px(1.5))
            .rounded(px(6.))
            .bg(alpha(t.yellow, 0.14))
            .child(icon(icon::LINK_OFF, 11., hsla(t.yellow)))
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(hsla(t.yellow))
                    .child(n.to_string()),
            )
            .into_any_element(),
        Trailing::Icon(i) => icon(*i, 14., hsla(t.fg_faint)).into_any_element(),
        Trailing::Kind(i) => icon(*i, 13., hsla(t.purple)).into_any_element(),
        Trailing::Chevron => icon(icon::CHEVRON_RIGHT, 14., hsla(t.fg_faint)).into_any_element(),
    }
}

pub fn divider(t: &Theme) -> AnyElement {
    div()
        .h(px(9.))
        .flex()
        .items_center()
        .px(px(14.))
        .child(div().h(px(1.)).w_full().bg(hsla(t.border)))
        .into_any_element()
}

pub fn caption(text: &SharedString, t: &Theme) -> AnyElement {
    div()
        .h(px(26.))
        .flex()
        .items_end()
        .px(px(14.))
        .pb(px(4.))
        .text_size(px(10.))
        .text_color(hsla(t.fg_faint))
        .child(text.to_string())
        .into_any_element()
}


pub fn claude_mark(size: f32) -> AnyElement {
    svg().path(CLAUDE_SVG).size(px(size)).text_color(hsla(CLAUDE)).into_any_element()
}
