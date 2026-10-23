use gpui::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div, px,
};
use mx_plugins::registry::{installed_banner, trust_facts, trust_headline};
use mx_plugins::{PluginInfo, StagedPlugin};

use super::widgets::{Close, barrier, card, text_button, title};
use crate::app::host::{StoreHost, update_store};
use crate::chrome::dialog::confirm::Confirm;
use crate::chrome::dialog::frame::BARRIER;
use crate::chrome::icon;
use crate::chrome::menu::Tone;
use crate::chrome::style::{hsla, theme};
use crate::chrome::widgets::{ButtonKind, TextField, TextFieldEvent};

pub const INSTALL_TITLE: &str = "Instalar plugin";

pub fn trust_title(name: &str, updating: bool) -> String {
    if updating { format!("Atualizar {name}?") } else { format!("Instalar {name}?") }
}

pub fn trust_button(updating: bool) -> &'static str {
    if updating { "Confio, atualizar" } else { "Confio, instalar" }
}

pub fn uninstall_text(linked: bool, dir: &str) -> String {
    if linked {
        let base = dir.trim_end_matches('/').rsplit('/').next().unwrap_or(dir);
        format!("Sai da MaestrIA o link pra {base}. A sua pasta de desenvolvimento fica onde está, intocada.")
    } else {
        "A pasta do plugin e o que ele guardou vão embora. Pra ter de volta, instale de novo.".to_owned()
    }
}

pub fn uninstall_confirm(p: &PluginInfo) -> Confirm {
    let text = uninstall_text(p.linked, &p.dir.to_string_lossy());
    Confirm {
        title: format!("Remover {}?", p.name()),
        width: 420.,
        path: None,
        facts: vec![(icon::DELETE_OUTLINE, Tone::Dim, text)],
        check: None,
        primary: "Remover".into(),
        kind: ButtonKind::Danger,
    }
}

enum Step {
    Ask,
    Reading,
    Trust(StagedPlugin),
    Installing,
}

pub struct InstallDialog {
    host: Entity<StoreHost>,
    close: Close,
    field: Entity<TextField>,
    step: Step,
    error: Option<String>,
    _sub: Subscription,
}

impl InstallDialog {
    pub fn new(host: Entity<StoreHost>, close: Close, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| TextField::new(cx).hint("URL do git, um .zip ou uma pasta").filled());
        field.read(cx).focus(window);
        let sub = cx.subscribe_in(&field, window, |this: &mut Self, _, ev: &TextFieldEvent, window, cx| match ev {
            TextFieldEvent::Submit => this.stage(cx),
            TextFieldEvent::Cancel => this.cancel(window, cx),
            TextFieldEvent::Changed(_) => {}
        });
        InstallDialog { host, close, field, step: Step::Ask, error: None, _sub: sub }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let (Step::Trust(staged), Some(r)) = (&self.step, self.host.read(cx).store.plugins()) {
            r.discard(staged);
        }
        (self.close)(window, cx);
    }

    fn stage(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.host.read(cx).store.plugins().cloned() else { return };
        let source = self.field.read(cx).text().to_owned();
        let task = self.host.read(cx).rt().spawn(async move { r.stage(&source).await });
        self.step = Step::Reading;
        self.error = None;
        cx.notify();
        cx.spawn(async move |me, cx| {
            let outcome = task.await;
            me.update(cx, |d, cx| {
                match outcome {
                    Ok(Ok(staged)) => d.step = Step::Trust(staged),
                    Ok(Err(e)) => {
                        d.step = Step::Ask;
                        d.error = Some(e.0);
                    }
                    Err(e) => {
                        d.step = Step::Ask;
                        d.error = Some(e.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Step::Trust(staged) = std::mem::replace(&mut self.step, Step::Installing) else { return };
        let Some(r) = self.host.read(cx).store.plugins().cloned() else { return };
        let task = self.host.read(cx).rt().spawn(async move {
            let out = r.commit(&staged).await;
            (out, staged)
        });
        cx.notify();
        let close = self.close.clone();
        let host = self.host.clone();
        let window_handle = window.window_handle();
        cx.spawn(async move |me, cx| {
            let Ok((outcome, staged)) = task.await else { return };
            match outcome {
                Ok(info) => {
                    let version = info.manifest.as_ref().map(|m| m.version.clone()).unwrap_or_default();
                    let banner = installed_banner(info.name(), &version, staged.replacing.as_deref());
                    cx.update(|cx| update_store(&host, cx, |s| s.plugin_installed(&info.id, &banner))).ok();
                    window_handle.update(cx, |_, window, cx| close(window, cx)).ok();
                }
                Err(e) => {
                    me.update(cx, |d, cx| {
                        d.step = Step::Ask;
                        d.error = Some(e.0);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        if let (Step::Trust(staged), Some(r)) = (&self.step, self.host.read(cx).store.plugins()) {
            r.discard(staged);
        }
        self.step = Step::Ask;
        cx.notify();
    }

    fn body(&self, cx: &mut Context<Self>) -> (String, AnyElement, Vec<AnyElement>) {
        let t = theme(cx);
        let me = cx.weak_entity();
        let cancel = {
            let me = me.clone();
            text_button("install-cancel", "Cancelar", hsla(t.accent), None, move |w, cx| {
                me.update(cx, |d, cx| d.cancel(w, cx)).ok();
            })
        };
        match &self.step {
            Step::Ask | Step::Reading => {
                let mut col = div().flex().flex_col().gap(px(8.)).child(self.field.clone());
                if matches!(self.step, Step::Reading) {
                    col = col.child(div().text_size(px(12.)).text_color(hsla(t.fg_faint)).child("Lendo…"));
                }
                if let Some(e) = &self.error {
                    col = col.child(div().text_size(px(12.)).text_color(hsla(t.red)).child(e.clone()));
                }
                let go = text_button("install-go", "Continuar", hsla(t.on_accent()), Some(hsla(t.accent)), move |_, cx| {
                    me.update(cx, |d, cx| d.stage(cx)).ok();
                });
                (INSTALL_TITLE.to_owned(), col.into_any_element(), vec![cancel, go])
            }
            Step::Trust(staged) => {
                let m = &staged.manifest;
                let updating = staged.replacing.is_some();
                let mut col = div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().font_family("Hack").text_size(px(11.5)).text_color(hsla(t.fg_dim)).child(trust_headline(m, staged.replacing.as_deref())))
                    .child(div().text_size(px(13.)).text_color(hsla(t.fg)).child(m.description.clone()));
                for f in trust_facts(m, crate::keymap::LINUX) {
                    col = col.child(div().text_size(px(12.5)).text_color(hsla(t.fg_dim)).child(format!("• {f}")));
                }
                let back = {
                    let me = me.clone();
                    text_button("install-back", "Voltar", hsla(t.accent), None, move |_, cx| {
                        me.update(cx, |d, cx| d.back(cx)).ok();
                    })
                };
                let go = text_button("install-trust", trust_button(updating), hsla(t.on_accent()), Some(hsla(t.accent)), move |w, cx| {
                    me.update(cx, |d, cx| d.commit(w, cx)).ok();
                });
                (trust_title(&m.name, updating), col.into_any_element(), vec![back, go])
            }
            Step::Installing => (INSTALL_TITLE.to_owned(), div().text_size(px(12.)).text_color(hsla(t.fg_faint)).child("Instalando…").into_any_element(), vec![]),
        }
    }
}

impl Render for InstallDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let (head, body, buttons) = self.body(cx);
        let me = cx.weak_entity();
        barrier(BARRIER, move |w, cx: &mut App| {
            me.update(cx, |d, cx| d.cancel(w, cx)).ok();
        })
        .flex()
        .items_center()
        .justify_center()
        .child(
            card(&t, 520.)
                .child(div().px(px(24.)).pt(px(18.)).pb(px(10.)).child(title(&t, head)))
                .child(div().px(px(24.)).pb(px(8.)).child(body))
                .child(div().px(px(16.)).py(px(12.)).flex().justify_end().gap(px(4.)).children(buttons)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trust_screen_says_install_or_update() {
        assert_eq!(trust_title("Docker", false), "Instalar Docker?");
        assert_eq!(trust_title("Docker", true), "Atualizar Docker?");
        assert_eq!(trust_button(false), "Confio, instalar");
        assert_eq!(trust_button(true), "Confio, atualizar");
    }

    #[test]
    fn removing_a_linked_plugin_keeps_the_dev_folder() {
        assert_eq!(
            uninstall_text(true, "/home/u/dev/meu-plugin"),
            "Sai da MaestrIA o link pra meu-plugin. A sua pasta de desenvolvimento fica onde está, intocada."
        );
        assert_eq!(uninstall_text(false, "/x"), "A pasta do plugin e o que ele guardou vão embora. Pra ter de volta, instale de novo.");
    }
}
