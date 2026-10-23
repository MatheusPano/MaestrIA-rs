use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window,
    div, px,
};

use super::frame::{alert, check_line, note};
use crate::chrome::icon::{Icon, icon};
use crate::chrome::menu::Tone;
use crate::chrome::style::{Handler, alpha, cb, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, TextField, TextFieldEvent};

#[derive(Clone, Debug, Default)]
pub struct FieldSpec {
    pub label: SharedString,
    pub hint: SharedString,
    pub initial: String,
    pub lines: Option<(usize, usize)>,
    pub mono: bool,
    pub row: Option<u8>,
    pub note: Option<SharedString>,
}

impl FieldSpec {
    pub fn new(label: impl Into<SharedString>, hint: impl Into<SharedString>) -> Self {
        Self { label: label.into(), hint: hint.into(), ..Default::default() }
    }

    pub fn initial(mut self, text: impl Into<String>) -> Self {
        self.initial = text.into();
        self
    }

    pub fn lines(mut self, min: usize, max: usize) -> Self {
        self.lines = Some((min, max));
        self
    }

    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn row(mut self, row: u8) -> Self {
        self.row = Some(row);
        self
    }

    pub fn note(mut self, text: impl Into<SharedString>) -> Self {
        self.note = Some(text.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct ActionSpec {
    pub label: SharedString,
    pub kind: ButtonKind,
}

impl ActionSpec {
    pub fn new(label: impl Into<SharedString>, kind: ButtonKind) -> Self {
        Self { label: label.into(), kind }
    }
}

pub type ChipSpec = (SharedString, Vec<(Icon, SharedString)>, usize);

#[derive(Clone, Debug, Default)]
pub struct FormSpec {
    pub title: SharedString,
    pub width: Option<f32>,
    pub fields: Vec<FieldSpec>,
    pub extras: Vec<(Icon, SharedString)>,
    pub note: Option<(SharedString, Tone)>,
    pub checks: Vec<(SharedString, bool)>,
    pub checks_max_height: f32,
    pub chips: Option<ChipSpec>,
    pub actions: Vec<ActionSpec>,
    pub submit: usize,
    pub required: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormResult {
    pub action: usize,
    pub fields: Vec<String>,
    pub checks: Vec<bool>,
    pub chip: Option<usize>,
}

pub type ExtraHandler = Rc<dyn Fn(usize, WeakEntity<FormDialog>, &mut Window, &mut App)>;

pub struct FormDialog {
    spec: FormSpec,
    fields: Vec<Entity<TextField>>,
    checks: Vec<bool>,
    chip: Option<usize>,
    note: Option<(SharedString, Tone)>,
    on_done: Handler<Option<FormResult>>,
    on_extra: Option<ExtraHandler>,
    _subs: Vec<Subscription>,
}

impl FormDialog {
    pub fn new(
        spec: FormSpec,
        on_done: impl Fn(Option<FormResult>, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subs = Vec::new();
        let fields: Vec<Entity<TextField>> = spec
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let field = cx.new(|cx| {
                    let mut tf = TextField::new(cx).label(f.label.clone()).hint(f.hint.clone()).with_text(f.initial.clone());
                    if let Some((min, max)) = f.lines {
                        tf = tf.lines(min, max);
                    }
                    if f.mono {
                        tf = tf.mono();
                    }
                    tf
                });
                subs.push(cx.subscribe_in(&field, window, move |this, _, ev: &TextFieldEvent, window, cx| match ev {
                    TextFieldEvent::Submit if i == 0 => this.finish(Some(this.spec.submit), window, cx),
                    TextFieldEvent::Cancel => this.finish(None, window, cx),
                    _ => cx.notify(),
                }));
                field
            })
            .collect();
        if let Some(first) = fields.first() {
            first.read(cx).focus(window);
        }
        Self {
            checks: spec.checks.iter().map(|(_, on)| *on).collect(),
            chip: spec.chips.as_ref().map(|(_, _, i)| *i),
            note: spec.note.clone(),
            spec,
            fields,
            on_done: Rc::new(on_done),
            on_extra: None,
            _subs: subs,
        }
    }

    pub fn on_extra(mut self, f: impl Fn(usize, WeakEntity<FormDialog>, &mut Window, &mut App) + 'static) -> Self {
        self.on_extra = Some(Rc::new(f));
        self
    }

    pub fn set_field(&mut self, index: usize, text: impl Into<String>, cx: &mut Context<Self>) {
        if let Some(f) = self.fields.get(index) {
            f.update(cx, |f, cx| f.set_text(text, cx));
        }
    }

    pub fn set_note(&mut self, text: impl Into<SharedString>, tone: Tone, cx: &mut Context<Self>) {
        self.note = Some((text.into(), tone));
        cx.notify();
    }

    pub fn field_text(&self, index: usize, cx: &App) -> String {
        self.fields.get(index).map(|f| f.read(cx).text().to_owned()).unwrap_or_default()
    }

    pub fn field(&self, index: usize) -> Option<&Entity<TextField>> {
        self.fields.get(index)
    }

    fn ready(&self, cx: &App) -> bool {
        self.spec.required.iter().all(|i| !self.field_text(*i, cx).trim().is_empty())
    }

    pub fn finish(&mut self, action: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let result = match action {
            Some(0) => None,
            Some(_) if !self.ready(cx) => return,
            Some(a) => Some(FormResult {
                action: a,
                fields: (0..self.fields.len()).map(|i| self.field_text(i, cx)).collect(),
                checks: self.checks.clone(),
                chip: self.chip,
            }),
            None => None,
        };
        (self.on_done)(result, window, cx);
    }

    fn fields_block(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = theme(cx);
        let mut out: Vec<AnyElement> = Vec::new();
        let mut i = 0;
        while i < self.fields.len() {
            let row = self.spec.fields[i].row;
            let mut j = i + 1;
            while row.is_some() && j < self.fields.len() && self.spec.fields[j].row == row {
                j += 1;
            }
            let mut line = div().flex().gap(px(10.)).mb(px(12.));
            for k in i..j {
                let mut cell = div().flex_1().child(self.fields[k].clone());
                if let Some(n) = &self.spec.fields[k].note {
                    cell = cell.child(div().pt(px(6.)).child(note(n.clone(), hsla(t.fg_faint))));
                }
                line = line.child(cell);
            }
            out.push(line.into_any_element());
            i = j;
        }
        out
    }
}

impl Render for FormDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let mut body = div().flex().flex_col().children(self.fields_block(cx));
        if !self.spec.extras.is_empty() {
            let extras = self.spec.extras.iter().enumerate().map(|(i, (glyph, label))| {
                Button::new(("extra", i), label.clone(), ButtonKind::Text)
                    .leading(*glyph)
                    .on_click({
                        let this = cx.entity().downgrade();
                        move |window, cx| {
                            let handler = this.upgrade().and_then(|e| e.read(cx).on_extra.clone());
                            if let Some(h) = handler {
                                h(i, this.clone(), window, cx);
                            }
                        }
                    })
            });
            body = body.child(div().flex().gap(px(4.)).mt(px(-2.)).mb(px(2.)).children(extras));
        }
        if let Some((text, tone)) = &self.note {
            body = body.child(note(text.clone(), hsla(tone.color(&t))));
        }
        if !self.spec.checks.is_empty() {
            let rows = self.spec.checks.iter().enumerate().map(|(i, (label, _))| {
                let on = self.checks[i];
                let this = cx.entity().downgrade();
                check_line(("form-check", i), label.clone(), on, move |_, cx| {
                    this.update(cx, |this, cx| {
                        this.checks[i] = !this.checks[i];
                        cx.notify();
                    })
                    .ok();
                }, &t)
            });
            body = body.child(
                div()
                    .id("form-checks")
                    .max_h(px(self.spec.checks_max_height.max(120.)))
                    .overflow_y_scroll()
                    .children(rows),
            );
        }
        if let Some((label, options, _)) = &self.spec.chips {
            let chips = options.iter().enumerate().map(|(i, (glyph, tip))| {
                let selected = self.chip == Some(i);
                let tip = tip.clone();
                div()
                    .id(("chip", i))
                    .debug_selector(move || format!("chip:{tip}"))
                    .size(px(34.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(7.))
                    .border_1()
                    .border_color(if selected { hsla(t.accent) } else { alpha(t.border, 0.) })
                    .when_bg(selected, hsla(t.bg_active))
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(t.bg_hover)))
                    .child(icon(*glyph, 17., hsla(if selected { t.fg } else { t.fg_dim })))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.chip = Some(i);
                        cx.notify();
                    }))
            });
            body = body
                .child(div().mt(px(12.)).mb(px(6.)).text_size(px(12.)).text_color(hsla(t.fg_dim)).child(label.clone()))
                .child(div().flex().flex_wrap().gap(px(6.)).children(chips));
        }
        let ready = self.ready(cx);
        let actions = self
            .spec
            .actions
            .iter()
            .enumerate()
            .map(|(i, a)| {
                Button::new(("action", i), a.label.clone(), a.kind)
                    .disabled(i != 0 && !ready)
                    .on_click(cb(cx.entity().downgrade(), move |this, window, cx| this.finish(Some(i), window, cx)))
                    .into_any_element()
            })
            .collect();
        let this = cx.entity().downgrade();
        alert(self.spec.title.clone(), self.spec.width, body, actions, Rc::new(move |window, cx| {
            this.update(cx, |this, cx| this.finish(None, window, cx)).ok();
        }), cx)
    }
}

trait WhenBg {
    fn when_bg(self, on: bool, color: gpui::Hsla) -> Self;
}

impl<E: Styled> WhenBg for E {
    fn when_bg(self, on: bool, color: gpui::Hsla) -> Self {
        if on { self.bg(color) } else { self }
    }
}
