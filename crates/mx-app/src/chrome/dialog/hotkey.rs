use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use super::frame::alert;
use crate::app::ui;
use crate::chrome::settings::KeyPress;
use crate::chrome::style::{cb, hsla, theme};
use crate::chrome::widgets::{Button, ButtonKind};
use crate::keymap::hotkey::{HotkeyCheck, judge_in};

const RECORD_PROMPT: &str = "Pressione a combinação   ⎋ cancela";

pub fn title(panel: &str) -> String {
    format!("Atalho de \"{panel}\"")
}

pub fn explain(current: Option<&str>) -> String {
    match current {
        None => "A combinação que traz este painel pra tela, de onde você estiver.".into(),
        Some(label) => format!("Hoje é {label}. Pressione outra pra trocar."),
    }
}

pub struct HotkeyDialog {
    tab: String,
    title: String,
    current: Option<String>,
    rejected: Option<String>,
    generation: u64,
    focus: FocusHandle,
    focused_once: bool,
}

pub fn open(tab: &str, _window: &mut Window, cx: &mut App) {
    let store = ui::store(cx);
    let Some(t) = store.tab(tab) else { return };
    let title = store.title(t);
    let current = t.hotkey.as_deref().map(mx_store::hotkey_label);
    let generation = ui::upcoming(cx);
    let tab = tab.to_owned();
    let view = cx.new(|cx| HotkeyDialog {
        tab,
        title,
        current,
        rejected: None,
        generation,
        focus: cx.focus_handle(),
        focused_once: false,
    });
    ui::modal(view.into(), cx);
}

impl HotkeyDialog {
    fn close(&mut self, cx: &mut Context<Self>) {
        ui::close_if(self.generation, cx);
    }

    fn pick(&mut self, chord: Option<String>, cx: &mut Context<Self>) {
        let tab = self.tab.clone();
        ui::act(cx, |s| s.set_hotkey(&tab, chord.as_deref()));
        self.close(cx);
    }

    pub fn press(&mut self, press: KeyPress, cx: &mut Context<Self>) {
        if press.key == "escape" && !(press.ctrl || press.alt || press.meta || press.shift) {
            self.close(cx);
            return;
        }
        match judge_in(ui::store(cx), &press) {
            HotkeyCheck::Ignore => {}
            HotkeyCheck::Rejected(why) => {
                self.rejected = Some(why);
                cx.notify();
            }
            HotkeyCheck::Accepted(id) => self.pick(Some(id), cx),
        }
    }

    fn key_down(&mut self, ev: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let press = KeyPress {
            key: k.key.clone(),
            ctrl: k.modifiers.control,
            alt: k.modifiers.alt,
            shift: k.modifiers.shift,
            meta: k.modifiers.platform,
        };
        self.press(press, cx);
        cx.stop_propagation();
    }
}

impl Render for HotkeyDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focused_once = true;
            self.focus.focus(window);
        }
        let t = theme(cx);
        let me = cx.entity().downgrade();
        let mut recorder = div()
            .debug_selector(|| "recorder".into())
            .flex()
            .items_center()
            .h(px(30.))
            .px(px(10.))
            .rounded(px(6.))
            .border_1()
            .border_color(hsla(t.accent))
            .child(div().text_size(px(11.)).text_color(hsla(t.accent)).child(RECORD_PROMPT));
        if self.rejected.is_none() {
            recorder = recorder.bg(hsla(t.accent.with_alpha(0.06)));
        }
        let mut body = div()
            .id("hotkey-body")
            .key_context("HotkeyRecorder")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .flex()
            .flex_col()
            .child(div().text_size(px(12.)).text_color(hsla(t.fg_dim)).child(explain(self.current.as_deref())))
            .child(div().h(px(12.)))
            .child(recorder);
        if let Some(why) = &self.rejected {
            let sel = why.clone();
            body = body.child(
                div()
                    .debug_selector(move || format!("rejection:{sel}"))
                    .mt(px(8.))
                    .text_size(px(11.))
                    .text_color(hsla(t.red))
                    .child(why.clone()),
            );
        }
        let mut actions: Vec<AnyElement> = Vec::new();
        if self.current.is_some() {
            let red = hsla(t.red);
            let hover = hsla(t.red.with_alpha(0.1));
            let remove = cb(me.clone(), |this, _, cx| this.pick(None, cx));
            actions.push(
                div()
                    .id("hotkey-remove")
                    .debug_selector(|| "button:Remover atalho".into())
                    .flex()
                    .items_center()
                    .h(px(34.))
                    .px(px(14.))
                    .rounded(px(18.))
                    .text_size(px(13.))
                    .text_color(red)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .on_click(move |_, window, cx| remove(window, cx))
                    .child("Remover atalho")
                    .into_any_element(),
            );
        }
        actions.push(
            Button::new("hotkey-cancel", "Cancelar", ButtonKind::Text)
                .on_click(cb(me.clone(), |this, _, cx| this.close(cx)))
                .into_any_element(),
        );
        alert(
            SharedString::from(title(&self.title)),
            Some(320.),
            body,
            actions,
            Rc::new(cb(me, |this, _, cx| this.close(cx))),
            cx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_says_what_the_key_does_or_which_key_it_has_today() {
        assert_eq!(title("primeiro"), "Atalho de \"primeiro\"");
        assert_eq!(explain(None), "A combinação que traz este painel pra tela, de onde você estiver.");
        assert_eq!(explain(Some("⌥⌘G")), "Hoje é ⌥⌘G. Pressione outra pra trocar.");
    }
}
