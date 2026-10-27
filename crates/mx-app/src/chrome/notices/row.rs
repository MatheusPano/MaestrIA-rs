use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, FontWeight, HighlightStyle, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, StyledText, div, px,
};
use mx_core::model::NoticeKind;
use mx_theme::{Color, Theme};

use crate::chrome::icon::{self, Icon, icon};
use crate::chrome::style::{Handler, alpha, hsla};
use crate::chrome::widgets::{IconButton, MxHover};

#[derive(Clone, Debug, PartialEq)]
pub struct NoticeView {
    pub id: u64,
    pub kind: NoticeKind,
    pub title: String,
    pub detail: Option<String>,
    pub read: bool,
    pub gone: bool,
    pub folder: Option<String>,
    pub ago: String,
}

pub fn kind_color(kind: NoticeKind, t: &Theme) -> Color {
    match kind {
        NoticeKind::Question => t.yellow,
        NoticeKind::Permission => t.red,
        NoticeKind::Finished => t.green,
        NoticeKind::Suggested => t.accent,
    }
}

pub fn kind_icon(kind: NoticeKind) -> Icon {
    match kind {
        NoticeKind::Question => icon::QUESTION_MARK_ROUNDED,
        NoticeKind::Permission => icon::LOCK_ROUNDED,
        NoticeKind::Finished => icon::CHECK_ROUNDED,
        NoticeKind::Suggested => icon::LIGHTBULB_OUTLINE_ROUNDED,
    }
}

pub fn sentence(n: &NoticeView) -> (String, Vec<Range<usize>>) {
    let mut text = n.title.clone();
    let title_end = text.len();
    let mut bold = Vec::new();
    bold.push(0..title_end);
    text.push(' ');
    text.push_str(n.kind.verb());
    if let Some(d) = &n.detail {
        text.push_str(": ");
        let at = text.len();
        text.push_str(d);
        bold.push(at..text.len());
    }
    (text, bold)
}

pub fn meta(n: &NoticeView) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(f) = &n.folder {
        parts.push(f);
    }
    if n.gone {
        parts.push("painel fechado");
    }
    parts.push(&n.ago);
    parts.join(" · ")
}

pub struct RowHandlers {
    pub on_open: Handler<u64>,
    pub on_close: Handler<u64>,
    pub on_hover: Option<Handler<(u64, bool)>>,
}

pub fn notice_row(n: &NoticeView, toast: bool, hovered: bool, h: &Rc<RowHandlers>, t: &Theme, _cx: &App) -> AnyElement {
    let kc = kind_color(n.kind, t);
    let (text, bold) = sentence(n);
    let weights: Vec<(Range<usize>, HighlightStyle)> = bold
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let weight = if i == 0 { FontWeight::BOLD } else { FontWeight::SEMIBOLD };
            (r, HighlightStyle { font_weight: Some(weight), color: Some(hsla(t.fg)), ..Default::default() })
        })
        .collect();
    let mut body = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(
            div()
                .text_size(px(12.5))
                .line_height(px(16.9))
                .text_color(hsla(if n.read { t.fg_dim } else { t.fg }))
                .child(StyledText::new(text).with_highlights(weights)),
        )
        .child(
            div()
                .text_size(px(11.))
                .text_color(hsla(t.fg_faint))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(meta(n)),
        );
    if n.gone {
        body = body.opacity(0.6);
    }
    let id = n.id;
    let trailing: AnyElement = if hovered || toast {
        let close = h.on_close.clone();
        IconButton::new(("notice-close", id as usize), icon::CLOSE_ROUNDED)
            .size(22., 15., 4.)
            .tooltip(if toast { "Tirar da tela" } else { "Dispensar" })
            .on_click(move |w, cx| close(id, w, cx))
            .into_any_element()
    } else if !n.read {
        div().size(px(22.)).flex().items_center().justify_center()
            .child(div().size(px(7.)).rounded(px(3.5)).bg(hsla(t.accent)))
            .into_any_element()
    } else {
        div().size(px(22.)).into_any_element()
    };
    let open = h.on_open.clone();
    let selector: SharedString = format!("notice:{}", n.title).into();
    let mut row = div()
        .id(("notice", id as usize))
        .debug_selector(move || selector.to_string())
        .flex()
        .items_start()
        .gap(px(10.))
        .cursor_pointer()
        .on_click(move |_, w, cx| open(id, w, cx))
        .child(
            div().size(px(20.)).flex_none().rounded(px(10.)).bg(alpha(kc, 0.16)).flex().items_center().justify_center()
                .child(icon(kind_icon(n.kind), 12., hsla(kc))),
        )
        .child(body)
        .child(trailing);
    row = if toast { row.pl(px(12.)).pr(px(6.)).py(px(10.)) } else { row.pl(px(6.)).pr(px(0.)).py(px(7.)).inset_hover(6., 3., 7., None) };
    if hovered && !n.gone {
        row = row.bg(hsla(t.bg_hover));
    }
    if let Some(hover) = h.on_hover.clone() {
        row = row.on_hover(move |on, w, cx| hover((id, *on), w, cx));
    }
    row.into_any_element()
}
