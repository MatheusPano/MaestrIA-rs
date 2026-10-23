use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use mx_core::model::FollowUpKind;

use super::cards::{add_tile, bullet, kind_tile, node};
use super::model::{self, Draft, FlowResult, Trigger};
use crate::chrome::dialog::frame::{barrier, card};
use crate::chrome::icon::{self, icon};
use crate::chrome::style::{Handler, MONO, cb, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind, IconButton, TextField, TextFieldEvent};

type Tool = Box<dyn Fn(&mut FlowEditor, &mut Context<FlowEditor>)>;

struct Step {
    draft: Draft,
    field: Entity<TextField>,
    _sub: Subscription,
}

pub struct FlowEditor {
    trigger: Trigger,
    targets: Vec<(String, String)>,
    steps: Vec<Step>,
    label: Option<Entity<TextField>>,
    prompt: Option<Entity<TextField>>,
    on_done: Handler<Option<FlowResult>>,
    _subs: Vec<Subscription>,
}

impl FlowEditor {
    pub fn new(
        trigger: Trigger,
        queued: Vec<Draft>,
        targets: Vec<(String, String)>,
        on_done: impl Fn(Option<FlowResult>, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subs = Vec::new();
        let (label, prompt) = if matches!(trigger, Trigger::New { .. }) {
            let label = cx.new(|cx| TextField::new(cx).hint("Como chamar o painel (opcional)").selector("flow-label").filled());
            let prompt = cx.new(|cx| TextField::new(cx).hint("O que pedir pra ela — o prompt de abertura").selector("flow-prompt").lines(3, 8).filled());
            for f in [&label, &prompt] {
                subs.push(cx.subscribe_in(f, window, |this, _, ev: &TextFieldEvent, window, cx| this.field_event(ev, window, cx)));
            }
            prompt.read(cx).focus(window);
            (Some(label), Some(prompt))
        } else {
            (None, None)
        };
        let mut this = Self { trigger, targets, steps: Vec::new(), label, prompt, on_done: Rc::new(on_done), _subs: subs };
        for d in queued {
            this.push(d, window, cx);
        }
        this
    }

    fn field_event(&mut self, ev: &TextFieldEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            TextFieldEvent::Cancel => (self.on_done)(None, window, cx),
            _ => cx.notify(),
        }
    }

    fn make_field(draft: &Draft, index_hint: usize, window: &mut Window, cx: &mut Context<Self>) -> (Entity<TextField>, Subscription) {
        let kind = draft.kind;
        let text = draft.text.clone();
        let field = cx.new(|cx| {
            let tf = TextField::new(cx).hint(kind.hint()).selector(format!("flow-step-{index_hint}")).with_text(text).filled();
            if kind == FollowUpKind::Command { tf.mono() } else { tf.lines(3, 8) }
        });
        let sub = cx.subscribe_in(&field, window, move |this, field, ev: &TextFieldEvent, window, cx| {
            if let TextFieldEvent::Changed(text) = ev
                && let Some(s) = this.steps.iter_mut().find(|s| &s.field == field)
            {
                s.draft.text = text.clone();
            }
            this.field_event(ev, window, cx);
        });
        (field, sub)
    }

    pub fn push(&mut self, draft: Draft, window: &mut Window, cx: &mut Context<Self>) {
        let (field, sub) = Self::make_field(&draft, self.steps.len(), window, cx);
        self.steps.push(Step { draft, field, _sub: sub });
        cx.notify();
    }

    pub fn add(&mut self, kind: FollowUpKind, window: &mut Window, cx: &mut Context<Self>) {
        self.push(Draft::new(kind), window, cx);
        if let Some(s) = self.steps.last() {
            s.field.read(cx).focus(window);
        }
    }

    pub fn set_kind(&mut self, i: usize, kind: FollowUpKind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(step) = self.steps.get_mut(i) else { return };
        step.draft.kind = kind;
        let draft = step.draft.clone();
        let (field, sub) = Self::make_field(&draft, i, window, cx);
        self.steps[i].field = field;
        self.steps[i]._sub = sub;
        cx.notify();
    }

    pub fn set_text(&mut self, i: usize, text: &str, cx: &mut Context<Self>) {
        if let Some(s) = self.steps.get_mut(i) {
            s.draft.text = text.to_owned();
            s.field.update(cx, |f, cx| f.set_text(text, cx));
        }
    }

    pub fn drafts(&self) -> Vec<Draft> {
        self.steps.iter().map(|s| s.draft.clone()).collect()
    }

    fn text(field: &Option<Entity<TextField>>, cx: &App) -> String {
        field.as_ref().map(|f| f.read(cx).text().to_owned()).unwrap_or_default()
    }

    pub fn verb(&self) -> &'static str {
        model::verb(&self.trigger, &self.drafts())
    }

    pub fn can_save(&self, cx: &App) -> bool {
        model::can_save(&self.trigger, &Self::text(&self.prompt, cx))
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_save(cx) {
            return;
        }
        let result = FlowResult {
            label: Self::text(&self.label, cx).trim().to_owned(),
            prompt: Self::text(&self.prompt, cx).trim().to_owned(),
            steps: model::ready(&self.drafts()),
        };
        (self.on_done)(Some(result), window, cx);
    }

    fn trigger_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let mut c = div().flex().flex_col().gap(px(3.)).p(px(12.)).rounded(px(9.)).bg(hsla(t.bg_active)).border_1().border_color(hsla(t.border));
        match &self.trigger {
            Trigger::New { place } => {
                c = c
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(format!("Abre uma sessão em {place}")))
                    .child(div().text_size(px(11.5)).text_color(hsla(t.fg_faint)).child("Ela é o primeiro passo: nasce com o prompt abaixo e o resto do fluxo já pendurado nela."))
                    .children(self.label.clone().map(|f| div().mt(px(6.)).child(f)))
                    .children(self.prompt.clone().map(|f| div().child(f)));
            }
            Trigger::Existing(s) => {
                let line = model::standing(s);
                let sel = line.clone();
                c = c
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child("Quando essa sessão terminar"))
                    .child(div().debug_selector(move || format!("standing:{sel}")).flex().items_center().gap(px(6.))
                        .child(div().size(px(7.)).rounded(px(3.5)).bg(hsla(s.tone.color(&t))))
                        .child(div().text_size(px(11.5)).text_color(hsla(t.fg_faint)).child(line)));
            }
        }
        node(bullet(crate::chrome::menu::claude_mark(13.), false, &t), true, c, &t)
    }

    fn step_card(&self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let t = theme(cx);
        let me = cx.entity().downgrade();
        let step = &self.steps[i];
        let n = self.steps.len();
        let tiles = model::kinds(!self.targets.is_empty()).into_iter().map(|k| {
            let on = step.draft.kind == k;
            kind_tile(i, k, on, cb(me.clone(), move |this, w, cx| this.set_kind(i, k, w, cx)), &t)
        });
        let tool = |id: &'static str, glyph, tip: &'static str, enabled: bool, f: Tool| {
            let me = me.clone();
            IconButton::new((id, i), glyph).size(24., 16., 6.).tooltip(tip).disabled(!enabled)
                .on_click(move |_, cx| { me.update(cx, |this, cx| { f(this, cx); cx.notify(); }).ok(); })
        };
        let mut col = div().flex().flex_col().p(px(12.)).rounded(px(9.)).bg(hsla(t.bg_sidebar)).border_1().border_color(hsla(t.border))
            .child(div().flex().items_start().gap(px(8.))
                .child(div().flex_1().flex().flex_wrap().gap(px(6.)).children(tiles))
                .child(tool("up", icon::KEYBOARD_ARROW_UP, "Subir", i > 0, Box::new(move |this, _| this.steps.swap(i, i - 1))))
                .child(tool("down", icon::KEYBOARD_ARROW_DOWN, "Descer", i + 1 < n, Box::new(move |this, _| this.steps.swap(i, i + 1))))
                .child(tool("drop", icon::CLOSE, "Tirar do fluxo", true, Box::new(move |this, _| { this.steps.remove(i); }))))
            .child(div().mt(px(8.)).text_size(px(11.5)).text_color(hsla(t.fg_faint)).child(step.draft.kind.blurb()));
        if step.draft.kind == FollowUpKind::Handoff {
            let chips = self.targets.iter().map(|(id, title)| {
                let on = step.draft.target.as_deref() == Some(id.as_str());
                let id = id.clone();
                div().id(gpui::SharedString::from(format!("target-{i}-{id}"))).px(px(8.)).py(px(3.)).rounded(px(6.)).cursor_pointer()
                    .border_1().border_color(if on { hsla(t.accent) } else { hsla(t.border) })
                    .text_size(px(12.)).text_color(hsla(t.fg)).child(title.clone())
                    .on_click(cx.listener(move |this, _, _, cx| { this.steps[i].draft.target = Some(id.clone()); cx.notify(); }))
            });
            col = col.child(div().mt(px(6.)).flex().flex_wrap().items_center().gap(px(6.))
                .child(icon(icon::ARROW_RIGHT_ALT, 16., hsla(t.fg_faint)))
                .child(div().text_size(px(12.)).text_color(hsla(t.fg_faint)).child("Pra qual painel"))
                .children(chips));
        }
        col = col.child(div().mt(px(2.)).child(step.field.clone()));
        let num = div().font_family(MONO).text_size(px(11.)).text_color(hsla(t.accent)).child((i + 1).to_string()).into_any_element();
        node(bullet(num, false, &t), true, col, &t)
    }
}

impl Render for FlowEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let me = cx.entity().downgrade();
        let vp = window.viewport_size();
        let w = 720f32.min(f32::from(vp.width) - 56.);
        let h = 640f32.min(f32::from(vp.height) - 56.);
        let fresh = matches!(self.trigger, Trigger::New { .. });
        let header = div().flex().items_center().gap(px(9.)).pl(px(20.)).pt(px(16.)).pr(px(16.)).pb(px(14.)).border_b_1().border_color(hsla(t.border))
            .child(icon(icon::ACCOUNT_TREE_OUTLINED, 16., hsla(t.purple)))
            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(if fresh { "Novo fluxo" } else { "Fluxo" }))
            .child(div().flex_1().ml(px(1.)).text_size(px(12.)).text_color(hsla(t.fg_faint)).whitespace_nowrap().overflow_hidden().text_ellipsis()
                .child(model::subtitle(&self.trigger, self.steps.len())))
            .child(IconButton::new("flow-close", icon::CLOSE).size(25., 15., 6.).on_click(cb(me.clone(), |this, w, cx| (this.on_done.clone())(None, w, cx))));
        let mut timeline = div().id("flow-timeline").flex_1().min_h_0().overflow_y_scroll().px(px(20.)).pt(px(18.)).pb(px(8.)).child(self.trigger_card(cx));
        for i in 0..self.steps.len() {
            timeline = timeline.child(self.step_card(i, cx));
        }
        let adds = model::kinds(!self.targets.is_empty()).into_iter().map(|k| add_tile(k, cb(me.clone(), move |this, w, cx| this.add(k, w, cx)), &t));
        let adder = div().pt(px(3.)).flex().flex_col().gap(px(8.))
            .child(div().text_size(px(12.)).text_color(hsla(t.fg_dim)).child(model::adder_title(self.steps.len())))
            .child(div().flex().flex_wrap().gap(px(8.)).children(adds));
        let plus = div().size(px(24.)).flex().items_center().justify_center().child(icon(icon::ADD, 13., hsla(t.fg_dim))).into_any_element();
        timeline = timeline.child(node(bullet(plus, true, &t), false, adder, &t));
        let footer = div().flex().items_center().gap(px(4.)).pl(px(20.)).pr(px(16.)).py(px(10.)).border_t_1().border_color(hsla(t.border))
            .child(div().flex_1().min_w_0().mr(px(10.)).text_size(px(11.)).line_height(px(14.85)).text_color(hsla(t.fg_faint)).child(model::rule(&self.trigger)))
            .child(Button::new("flow-cancel", "Cancelar", ButtonKind::Text).on_click(cb(me.clone(), |this, w, cx| (this.on_done.clone())(None, w, cx))))
            .child(Button::new("flow-save", self.verb(), ButtonKind::Primary).disabled(!self.can_save(cx)).on_click(cb(me.clone(), |this, w, cx| this.save(w, cx))));
        let close = Rc::new(cb(me, |this, w, cx| (this.on_done.clone())(None, w, cx)));
        barrier(0x0000_0066, Some(close)).child(
            card(&t, 12.).w(px(w)).max_h(px(h)).child(header).child(timeline).child(footer),
        )
    }
}
