use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use mx_plugins::ui::dialogs::{InputRequest, PICK_EMPTY, PickItem, PickRequest, cycle_highlight};
use mx_store::DialogAnswer;

use super::widgets::{PICK_BARRIER, answer, barrier, card, dim};
use crate::app::host::StoreHost;
use crate::chrome::style::{hsla, theme};
use crate::chrome::widgets::{MxHover, TextField, TextFieldEvent};

fn place(body: impl IntoElement, host: Entity<StoreHost>) -> impl IntoElement {
    barrier(PICK_BARRIER, move |_, cx| answer(&host, DialogAnswer::Text(None), cx))
        .flex()
        .flex_col()
        .items_center()
        .child(div().h(gpui::relative(0.2)).flex_none())
        .child(body)
}

pub struct PickDialog {
    host: Entity<StoreHost>,
    req: PickRequest,
    field: Entity<TextField>,
    query: String,
    highlight: usize,
    _sub: Subscription,
}

impl PickDialog {
    pub fn new(host: Entity<StoreHost>, req: PickRequest, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hint = req.hint().to_owned();
        let field = cx.new(|cx| TextField::new(cx).hint(hint).filled());
        field.read(cx).focus(window);
        let sub = cx.subscribe(&field, |this: &mut Self, _, ev: &TextFieldEvent, cx| match ev {
            TextFieldEvent::Changed(q) => {
                this.query = q.clone();
                this.highlight = 0;
                cx.notify();
            }
            TextFieldEvent::Submit => this.pick_highlighted(cx),
            TextFieldEvent::Cancel => answer(&this.host, DialogAnswer::Text(None), cx),
        });
        PickDialog { host, req, field, query: String::new(), highlight: 0, _sub: sub }
    }

    fn shown(&self) -> Vec<PickItem> {
        self.req.filter(&self.query).into_iter().cloned().collect()
    }

    fn pick_highlighted(&mut self, cx: &mut Context<Self>) {
        let value = self.shown().get(self.highlight).map(|i| i.value.clone());
        if value.is_some() {
            answer(&self.host, DialogAnswer::Text(value), cx);
        }
    }

    fn key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = match ev.keystroke.key.as_str() {
            "up" => -1,
            "down" => 1,
            _ => return,
        };
        self.highlight = cycle_highlight(self.highlight, delta, self.shown().len());
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for PickDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let items = self.shown();
        let mut list = div().id("pick-list").max_h(px(360.)).overflow_y_scroll().flex().flex_col().py(px(4.));
        if items.is_empty() {
            list = list.child(div().px(px(16.)).py(px(10.)).text_size(px(13.)).text_color(hsla(t.fg_faint)).child(PICK_EMPTY));
        }
        for (i, item) in items.into_iter().enumerate() {
            let on = i == self.highlight;
            let host = self.host.clone();
            let value = item.value.clone();
            list = list.child(
                div()
                    .id(("pick", i))
                    .px(px(10.))
                    .py(px(7.))
                    .inset_hover(6., 0., 6., None)
                    .flex()
                    .flex_col()
                    .cursor_pointer()
                    .when(on, |d| d.bg(hsla(t.bg_active)))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if *hovered && this.highlight != i {
                            this.highlight = i;
                            cx.notify();
                        }
                    }))
                    .on_click(move |_, _, cx| answer(&host, DialogAnswer::Text(Some(value.clone())), cx))
                    .child(div().text_size(px(13.5)).text_color(hsla(t.fg)).child(item.label.clone()))
                    .when_some(item.detail.clone(), |d, detail| {
                        d.child(div().min_w_0().truncate().text_size(px(12.)).text_color(hsla(t.fg_faint)).child(detail))
                    }),
            );
        }
        let body = card(&t, 520.)
            .capture_key_down(cx.listener(Self::key))
            .child(div().px(px(16.)).pt(px(12.)).child(dim(&t, SharedString::from(self.req.title.clone()))))
            .child(div().px(px(12.)).py(px(8.)).child(self.field.clone()))
            .child(div().border_t_1().border_color(hsla(t.border)).child(list));
        place(body, self.host.clone())
    }
}

pub struct InputDialog {
    host: Entity<StoreHost>,
    req: InputRequest,
    field: Entity<TextField>,
    _sub: Subscription,
}

impl InputDialog {
    pub fn new(host: Entity<StoreHost>, req: InputRequest, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hint = req.placeholder.clone().unwrap_or_default();
        let value = req.value.clone().unwrap_or_default();
        let field = cx.new(|cx| {
            let mut f = TextField::new(cx).hint(hint).filled().with_text(value);
            f.select_all(cx);
            f
        });
        field.read(cx).focus(window);
        let sub = cx.subscribe(&field, |this: &mut Self, f, ev: &TextFieldEvent, cx| match ev {
            TextFieldEvent::Submit => {
                let typed = f.read(cx).text().to_owned();
                answer(&this.host, DialogAnswer::Text(InputRequest::answer(&typed)), cx);
            }
            TextFieldEvent::Cancel => answer(&this.host, DialogAnswer::Text(None), cx),
            TextFieldEvent::Changed(_) => {}
        });
        InputDialog { host, req, field, _sub: sub }
    }
}

impl Render for InputDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let body = card(&t, 520.)
            .child(div().px(px(16.)).pt(px(12.)).child(dim(&t, SharedString::from(self.req.title.clone()))))
            .child(div().px(px(12.)).py(px(8.)).child(self.field.clone()))
            .child(
                div()
                    .px(px(16.))
                    .pb(px(12.))
                    .text_size(px(11.5))
                    .text_color(hsla(t.fg_faint))
                    .child(self.req.help().to_owned()),
            );
        place(body, self.host.clone())
    }
}
