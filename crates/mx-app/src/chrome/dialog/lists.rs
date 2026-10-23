use std::rc::Rc;

use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px, svg,
};
use mx_core::model::FeatureOrHotfixKind;
use mx_theme::CLAUDE;

use super::frame::{alert, barrier, card, BARRIER};
use crate::chrome::assets::CLAUDE_SVG;
use crate::chrome::icon::{self, icon};
use crate::chrome::menu::Tone;
use crate::chrome::menu::actions::kind_icon;
use crate::chrome::style::{Callback, Handler, MONO, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind};

#[derive(Clone, Debug)]
pub struct MoveOption {
    pub id: String,
    pub name: String,
    pub kind: FeatureOrHotfixKind,
    pub current: bool,
}

#[derive(IntoElement)]
pub struct MoveToDialog {
    pub title: String,
    pub options: Vec<MoveOption>,
    pub loose: bool,
    pub on_pick: Handler<Option<String>>,
    pub on_cancel: Callback,
}

pub fn move_title(tab_title: &str) -> String {
    format!("Mover \"{tab_title}\" pra…")
}

pub fn none_label(loose: bool) -> &'static str {
    if loose { "Nenhum — solto na bandeja" } else { "Nenhum — solto na pasta" }
}

pub fn no_features_banner(loose: bool) -> &'static str {
    if loose {
        "Os avulsos ainda não têm feature nem hotfix — crie pelo + da bandeja"
    } else {
        "Essa pasta ainda não tem feature nem hotfix — crie pelo + da pasta"
    }
}

impl RenderOnce for MoveToDialog {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let option = |i: usize, glyph, color, label: String, label_color, value: Option<String>, pick: Handler<Option<String>>| {
            let selector = label.clone();
            div()
                .id(("move", i))
                .debug_selector(move || format!("option:{selector}"))
                .flex()
                .items_center()
                .gap(px(9.))
                .px(px(24.))
                .py(px(10.))
                .cursor_pointer()
                .hover(|s| s.bg(hsla(t.bg_hover)))
                .child(icon(glyph, 15., color))
                .child(div().text_size(px(13.)).text_color(label_color).child(label))
                .on_click(move |_, window, cx| pick(value.clone(), window, cx))
        };
        let mut rows: Vec<AnyElement> = self
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let color = if o.current { t.accent } else { t.purple };
                option(i, kind_icon(o.kind), hsla(color), o.name.clone(), hsla(t.fg), Some(o.id.clone()), self.on_pick.clone()).into_any_element()
            })
            .collect();
        rows.push(
            option(999, icon::REMOVE_CIRCLE_OUTLINE, hsla(t.fg_faint), none_label(self.loose).into(), hsla(t.fg_dim), None, self.on_pick.clone())
                .into_any_element(),
        );
        let cancel = self.on_cancel.clone();
        barrier(BARRIER, Some(cancel)).child(
            card(&t, 12.)
                .min_w(px(280.))
                .pb(px(16.))
                .child(div().px(px(24.)).pt(px(24.)).pb(px(12.)).text_size(px(15.)).text_color(hsla(t.fg)).child(self.title))
                .children(rows),
        )
    }
}

#[derive(Clone, Debug)]
pub struct ChatRow {
    pub title: String,
    pub meta: String,
    pub note: Option<(String, Tone)>,
    pub gone: bool,
}

#[derive(Clone, Debug)]
pub enum More {
    Offer(String),
    Loading,
}

#[derive(Clone, Debug)]
pub struct ChatSection {
    pub name: String,
    pub rows: Vec<ChatRow>,
    pub more: Option<More>,
}

#[derive(IntoElement)]
pub struct ChatHistoryDialog {
    pub loading: bool,
    pub sections: Vec<ChatSection>,
    pub on_resume: Handler<(usize, usize)>,
    pub on_more: Handler<usize>,
    pub on_close: Callback,
}

impl RenderOnce for ChatHistoryDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme(cx);
        let screen = window.viewport_size();
        let width = 760f32.min(f32::from(screen.width) - 112.);
        let max_h = 620f32.min(f32::from(screen.height) - 260.);
        let body: AnyElement = if self.loading {
            div().h(px(90.)).flex().items_center().justify_center()
                .child(crate::chrome::anim::Spinner::new(15., t.accent))
                .into_any_element()
        } else if self.sections.is_empty() {
            div().text_size(px(12.)).text_color(hsla(t.fg_dim)).child("O Claude não guardou nenhuma conversa ainda.").into_any_element()
        } else {
            let mut list = div().id("chat-list").max_h(px(max_h)).overflow_y_scroll().flex().flex_col();
            for (si, s) in self.sections.iter().enumerate() {
                list = list.child(
                    div().flex().items_center().gap(px(9.)).pl(px(10.)).pr(px(12.)).pt(px(if si == 0 { 2. } else { 18. })).pb(px(5.))
                        .child(div().text_size(px(10.5)).text_color(hsla(t.fg_faint)).child(s.name.clone()))
                        .child(div().flex_1().h(px(1.)).bg(hsla(t.border)))
                        .child(div().text_size(px(11.)).text_color(hsla(t.fg_faint)).child(s.rows.len().to_string())),
                );
                for (ri, r) in s.rows.iter().enumerate() {
                    let pick = self.on_resume.clone();
                    let mark: AnyElement = if r.gone {
                        icon(icon::LINK_OFF, 13., hsla(t.yellow)).into_any_element()
                    } else {
                        svg().path(CLAUDE_SVG).size(px(13.)).text_color(hsla(CLAUDE)).into_any_element()
                    };
                    let mut meta = div().flex().items_center().gap(px(8.)).child(
                        div().flex_1().min_w_0().font_family(MONO).text_size(px(10.5)).text_color(hsla(t.fg_faint)).whitespace_nowrap().overflow_hidden().text_ellipsis().child(r.meta.clone()),
                    );
                    if let Some((n, tone)) = &r.note {
                        meta = meta.child(div().text_size(px(10.5)).text_color(hsla(tone.color(&t))).child(n.clone()));
                    }
                    list = list.child(
                        div().id(SharedString::from(format!("chat-{si}-{ri}"))).flex().gap(px(9.)).pl(px(10.)).pr(px(12.)).py(px(7.))
                            .cursor_pointer().hover(|s| s.bg(hsla(t.bg_hover)))
                            .on_click(move |_, w, cx| pick((si, ri), w, cx))
                            .child(div().pt(px(2.)).child(mark))
                            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(2.))
                                .child(div().text_size(px(12.5)).text_color(hsla(if r.gone { t.fg_dim } else { t.fg })).whitespace_nowrap().overflow_hidden().text_ellipsis().child(r.title.clone()))
                                .child(meta)),
                    );
                }
                if let Some(more) = &s.more {
                    let (glyph, label, on) = match more {
                        More::Offer(l) => (icon::UNFOLD_MORE, l.clone(), true),
                        More::Loading => (icon::MORE_HORIZ, "Buscando…".to_owned(), false),
                    };
                    let f = self.on_more.clone();
                    let mut row = div().id(("more", si)).flex().items_center().gap(px(9.)).pl(px(10.)).pr(px(12.)).py(px(7.))
                        .child(icon(glyph, 13., hsla(t.fg_faint)))
                        .child(div().text_size(px(11.5)).text_color(hsla(t.fg_dim)).child(label));
                    if on {
                        row = row.cursor_pointer().hover(|s| s.bg(hsla(t.bg_hover))).on_click(move |_, w, cx| f(si, w, cx));
                    }
                    list = list.child(row);
                }
            }
            list.into_any_element()
        };
        let close = self.on_close.clone();
        let actions = vec![Button::new("close", "Fechar", ButtonKind::Text).callback(Some(close)).into_any_element()];
        alert("Todas as conversas", Some(width), body, actions, Rc::clone(&self.on_close), cx)
    }
}
