use std::rc::Rc;

use gpui::{
    AnyElement, AppContext, Context, Entity, InteractiveElement, IntoElement, KeyDownEvent, StatefulInteractiveElement,
    ParentElement, SharedString, Styled, Window, anchored, deferred, div, point, px,
};
use mx_claude::setup::{SetupSection, SetupSlot};

use super::SetupView;
use super::state::{delete_detail, prompt_label};
use super::view::hsla;
use crate::chrome::dialog::form::FormDialog;
use crate::chrome::dialog::frame::{BARRIER, alert, barrier, card};
use crate::chrome::dialog::specs;
use crate::chrome::style::{MONO, cb};
use crate::chrome::widgets::{Button, ButtonKind};
use crate::reader::markdown_body;

pub enum Pending {
    Select(String),
    Revert,
    Create { rel: String, name: String },
}

pub enum Overlay {
    Help(SetupSection),
    Discard(String, Pending),
    Delete(SetupSlot),
    Prompt(Entity<FormDialog>),
}

impl SetupView {
    pub fn explain(&mut self, section: SetupSection, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = Some(Overlay::Help(section));
        window.focus(&self.focus);
        cx.notify();
    }

    pub(super) fn ask_discard(&mut self, pending: Pending, cx: &mut Context<Self>) {
        let label = self.slot().map(|s| s.label).unwrap_or_default();
        self.overlay = Some(Overlay::Discard(label, pending));
        cx.notify();
    }

    pub(super) fn open_prompt(&mut self, section: SetupSection, window: &mut Window, cx: &mut Context<Self>) {
        let me = cx.entity().downgrade();
        let spec = specs::prompt_text(&format!("Nova {}", section.noun()), prompt_label(section), "");
        let form = cx.new(|cx| {
            FormDialog::new(
                spec,
                move |result, window, cx| {
                    me.update(cx, |this, cx| {
                        this.dismiss(cx);
                        if let Some(r) = result.filter(|r| r.action == 1) {
                            let name = r.fields.first().cloned().unwrap_or_default();
                            this.named(section, name, window, cx);
                        }
                    })
                    .ok();
                },
                window,
                cx,
            )
        });
        self.overlay = Some(Overlay::Prompt(form));
        cx.notify();
    }

    pub(super) fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.overlay.take().is_some() {
            cx.notify();
        }
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.overlay.take() {
            Some(Overlay::Discard(_, pending)) => self.resume(pending, window, cx),
            Some(Overlay::Delete(slot)) => self.delete_now(slot, window, cx),
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn overlay_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev.keystroke.key.as_str() {
            "escape" => self.dismiss(cx),
            "enter" if !matches!(self.overlay, Some(Overlay::Help(_))) => self.confirm(window, cx),
            "enter" => self.dismiss(cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    pub(super) fn render_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let me = cx.entity().downgrade();
        let t = self.theme.clone();
        let close = Rc::new(cb(me.clone(), |this, _, cx| this.dismiss(cx)));
        let content: AnyElement = match self.overlay.as_ref()? {
            Overlay::Prompt(form) => form.clone().into_any_element(),
            Overlay::Help(section) => {
                let h = (f32::from(window.viewport_size().height) * 0.7).clamp(240., 680.);
                let section = *section;
                let help = div()
                    .id("setup-help")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(26.))
                    .py(px(18.))
                    .child(markdown_body(section.help(), &t, -0.5, 608., window));
                let head = div()
                    .px(px(26.))
                    .pt(px(16.))
                    .flex()
                    .items_baseline()
                    .gap(px(8.))
                    .child(div().text_size(px(15.)).font_weight(gpui::FontWeight::BOLD).text_color(hsla(t.fg)).child(section.label()))
                    .child(div().text_size(px(12.)).text_color(hsla(t.fg_dim)).child(format!("O que é {}?", section.label())));
                barrier(BARRIER, Some(close))
                    .child(
                        card(&t, 12.)
                            .w(px(660.))
                            .child(div().h(px(h)).flex().flex_col().child(head).child(help))
                            .child(
                                div().flex().justify_end().px(px(20.)).py(px(14.)).border_t_1().border_color(hsla(t.border)).child(
                                    Button::new("setup-help-ok", "Entendi", ButtonKind::Primary)
                                        .on_click(cb(me.clone(), |this, _, cx| this.dismiss(cx))),
                                ),
                            ),
                    )
                    .into_any_element()
            }
            Overlay::Discard(label, _) => {
                let body = div()
                    .text_size(px(12.5))
                    .text_color(hsla(t.fg_dim))
                    .child("O arquivo não foi salvo. Sair dele agora perde o que você escreveu.");
                let actions = vec![
                    Button::new("setup-back", "Voltar", ButtonKind::Text).on_click(cb(me.clone(), |this, _, cx| this.dismiss(cx))).into_any_element(),
                    Button::new("setup-discard", "Descartar", ButtonKind::Danger).on_click(cb(me.clone(), |this, w, cx| this.confirm(w, cx))).into_any_element(),
                ];
                alert(SharedString::from(format!("Descartar o que mudou em {label}?")), Some(400.), body, actions, close, cx).into_any_element()
            }
            Overlay::Delete(slot) => {
                let body = div()
                    .font_family(MONO)
                    .text_size(px(11.))
                    .text_color(hsla(t.fg_faint))
                    .child(delete_detail(slot, &self.setup.root));
                let actions = vec![
                    Button::new("setup-cancel", "Cancelar", ButtonKind::Text).on_click(cb(me.clone(), |this, _, cx| this.dismiss(cx))).into_any_element(),
                    Button::new("setup-delete", "Apagar", ButtonKind::Danger).on_click(cb(me.clone(), |this, w, cx| this.confirm(w, cx))).into_any_element(),
                ];
                alert(SharedString::from(format!("Apagar {}?", slot.label)), Some(420.), body, actions, close, cx).into_any_element()
            }
        };
        let keyed = !matches!(self.overlay, Some(Overlay::Prompt(_)));
        if keyed && !self.focus.is_focused(window) {
            window.focus(&self.focus);
        }
        let size = window.viewport_size();
        let layer = div().relative().w(size.width).h(size.height).child(content);
        Some(deferred(anchored().position(point(px(0.), px(0.))).child(layer)).with_priority(2).into_any_element())
    }
}
