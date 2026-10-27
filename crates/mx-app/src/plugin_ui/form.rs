use std::collections::HashMap;

use gpui::{
    AnyElement, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div, prelude::FluentBuilder, px,
};
use mx_plugins::ui::form::{FormButton, FormField, FormPress, PluginForm};
use mx_store::DialogAnswer;

use super::widgets::{answer, barrier, card, chip, text_button, title};
use crate::app::host::StoreHost;
use crate::chrome::dialog::frame::BARRIER;
use crate::chrome::style::{hsla, theme};
use crate::chrome::widgets::{TextField, TextFieldEvent};

pub struct FormDialog {
    host: Entity<StoreHost>,
    form: PluginForm,
    values: Vec<(String, String)>,
    fields: HashMap<String, Entity<TextField>>,
    missing: Option<(String, String)>,
    _subs: Vec<Subscription>,
}

impl FormDialog {
    pub fn new(host: Entity<StoreHost>, form: PluginForm, _plugin: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let values = form.initial_values();
        let mut fields = HashMap::new();
        let mut subs = Vec::new();
        let mut first = true;
        for f in form.fields.iter().filter(|f| !f.select) {
            let hint = f.placeholder.clone().unwrap_or_default();
            let (label, text) = (f.label.clone(), f.initial());
            let e = cx.new(|cx| {
                let field = TextField::new(cx).hint(hint).filled().with_text(text);
                match label {
                    Some(l) => field.label(l),
                    None => field,
                }
            });
            if first {
                e.read(cx).focus(window);
                first = false;
            }
            let id = f.id.clone();
            subs.push(cx.subscribe(&e, move |this: &mut Self, _, ev: &TextFieldEvent, cx| match ev {
                TextFieldEvent::Changed(text) => {
                    this.set(&id, text.clone());
                    cx.notify();
                }
                TextFieldEvent::Submit => {
                    if let Some(b) = this.form.enter_button().cloned() {
                        this.press(&b, cx);
                    }
                }
                TextFieldEvent::Cancel => answer(&this.host, DialogAnswer::Form(None), cx),
            }));
            fields.insert(f.id.clone(), e);
        }
        FormDialog { host, form, values, fields, missing: None, _subs: subs }
    }

    fn set(&mut self, id: &str, value: String) {
        if let Some(slot) = self.values.iter_mut().find(|(k, _)| k == id) {
            slot.1 = value;
        }
        if self.missing.as_ref().is_some_and(|(f, _)| f == id) {
            self.missing = None;
        }
    }

    fn value(&self, id: &str) -> &str {
        self.values.iter().find(|(k, _)| k == id).map_or("", |(_, v)| v.as_str())
    }

    fn press(&mut self, button: &FormButton, cx: &mut Context<Self>) {
        match self.form.press(button, &self.values) {
            FormPress::Submit(a) => answer(&self.host, DialogAnswer::Form(Some(a)), cx),
            FormPress::Missing { field, text } => {
                self.missing = Some((field, text));
                cx.notify();
            }
        }
    }

    fn field(&self, f: &FormField, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let mut col = div().flex().flex_col().gap(px(4.)).flex_1().min_w_0();
        if f.select {
            if let Some(label) = &f.label {
                col = col.child(div().text_size(px(12.)).text_color(hsla(t.fg_dim)).child(label.clone()));
            }
            let current = self.value(&f.id).to_owned();
            let mut chips = div().flex().flex_wrap().gap(px(6.));
            for (value, label) in &f.options {
                let me = cx.weak_entity();
                let (id, v) = (f.id.clone(), value.clone());
                chips = chips.child(chip(format!("{}={value}", f.id), label.clone(), *value == current, &t, move |_, cx| {
                    me.update(cx, |d, cx| {
                        d.set(&id, v.clone());
                        cx.notify();
                    })
                    .ok();
                }));
            }
            col = col.child(chips);
        } else if let Some(e) = self.fields.get(&f.id) {
            col = col.child(e.clone());
        }
        let missing = self.missing.as_ref().filter(|(id, _)| *id == f.id).map(|(_, text)| text.clone());
        col.when_some(missing, |d, text| d.child(div().text_size(px(11.5)).text_color(hsla(t.red)).child(text)))
            .into_any_element()
    }

    fn button(&self, b: &FormButton, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let me = cx.weak_entity();
        let pressed = b.clone();
        let (color, filled) = if b.danger() {
            (hsla(t.red), None)
        } else if b.primary() {
            (hsla(t.on_accent()), Some(hsla(t.accent)))
        } else {
            (hsla(t.accent), None)
        };
        text_button(format!("form-{}", b.action), b.label.clone(), color, filled, move |_, cx| {
            me.update(cx, |d, cx| d.press(&pressed, cx)).ok();
        })
    }
}

impl Render for FormDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let rows: Vec<Vec<FormField>> = self.form.rows(&self.values).into_iter().map(|r| r.into_iter().cloned().collect()).collect();
        let mut body = div().flex().flex_col().gap(px(12.));
        for row in rows {
            let lone_half = row.len() == 1 && row[0].half;
            let mut line = div().flex().gap(px(12.));
            for f in &row {
                line = line.child(self.field(f, cx));
            }
            if lone_half {
                line = line.child(div().flex_1());
            }
            body = body.child(line);
        }
        if let Some(e) = &self.form.error {
            body = body.child(div().text_size(px(12.)).text_color(hsla(t.red)).child(e.clone()));
        }
        let buttons = self.form.buttons.clone();
        let left: Vec<AnyElement> = buttons.iter().filter(|b| b.danger()).map(|b| self.button(b, cx)).collect();
        let host = self.host.clone();
        let mut right = vec![text_button("form-cancel", PluginForm::CANCEL, hsla(t.accent), None, move |_, cx| {
            answer(&host, DialogAnswer::Form(None), cx)
        })];
        right.extend(buttons.iter().filter(|b| !b.danger()).map(|b| self.button(b, cx)));
        let dismiss = self.host.clone();
        barrier(BARRIER, move |_, cx| answer(&dismiss, DialogAnswer::Form(None), cx))
            .flex()
            .items_center()
            .justify_center()
            .child(
                card(&t, 480.)
                    .child(div().px(px(24.)).pt(px(18.)).pb(px(10.)).child(title(&t, self.form.title.clone())))
                    .child(div().px(px(24.)).pb(px(8.)).child(body))
                    .child(
                        div()
                            .px(px(16.))
                            .py(px(12.))
                            .flex()
                            .items_center()
                            .child(div().flex().gap(px(4.)).children(left))
                            .child(div().flex_1())
                            .child(div().flex().gap(px(4.)).children(right)),
                    ),
            )
    }
}
