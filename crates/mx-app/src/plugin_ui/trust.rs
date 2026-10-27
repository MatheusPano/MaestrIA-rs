use gpui::{
    App, AsyncApp, Context, Div, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, Styled, WeakEntity, Window, div, px,
};
use mx_plugins::catalog::{PluginUpdate, PluginUpdates};
use mx_plugins::registry::{installed_banner, trust_facts, trust_headline};
use mx_plugins::{InstallError, PluginInfo, StagedPlugin};
use mx_theme::Theme;

use super::PluginLayer;
use super::widgets::{Close, barrier, card, text_button, title};
use crate::app::host::{StoreHost, update_store};
use crate::chrome::dialog::frame::BARRIER;
use crate::chrome::style::{hsla, theme};

pub type Finished = (Result<PluginInfo, InstallError>, StagedPlugin);

pub fn trust_title(name: &str, updating: bool) -> String {
    if updating { format!("Atualizar {name}?") } else { format!("Instalar {name}?") }
}

pub fn trust_button(updating: bool) -> &'static str {
    if updating { "Confio, atualizar" } else { "Confio, instalar" }
}

pub fn trust_body(staged: &StagedPlugin, t: &Theme) -> Div {
    let m = &staged.manifest;
    let mut col = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().font_family("Hack").text_size(px(11.5)).text_color(hsla(t.fg_dim)).child(trust_headline(m, staged.replacing.as_deref())))
        .child(div().text_size(px(13.)).text_color(hsla(t.fg)).child(m.description.clone()));
    for f in trust_facts(m, crate::keymap::LINUX) {
        col = col.child(div().text_size(px(12.5)).text_color(hsla(t.fg_dim)).child(format!("• {f}")));
    }
    col
}

pub fn finish(host: &Entity<StoreHost>, staged: StagedPlugin, cx: &App) -> tokio::task::JoinHandle<Finished> {
    let h = host.read(cx);
    let updates = h.store.plugin_updates().cloned();
    let registry = h.store.plugins().cloned();
    h.rt().spawn(async move {
        let out = match (updates, registry) {
            (Some(u), _) => u.finish(&staged).await,
            (None, Some(r)) => r.commit(&staged).await,
            (None, None) => Err(InstallError("Os plugins não estão carregados".into())),
        };
        (out, staged)
    })
}

pub fn announce(host: &Entity<StoreHost>, info: &PluginInfo, staged: &StagedPlugin, cx: &mut App) {
    let version = info.manifest.as_ref().map(|m| m.version.clone()).unwrap_or_default();
    let banner = installed_banner(info.name(), &version, staged.replacing.as_deref());
    update_store(host, cx, |s| s.plugin_installed(&info.id, &banner));
}

pub fn discard(host: &Entity<StoreHost>, staged: &StagedPlugin, cx: &App) {
    if let Some(r) = host.read(cx).store.plugins() {
        r.discard(staged);
    }
}

pub fn download_failed(to: &str, e: &str) -> String {
    format!("Não deu pra baixar a {to}: {e}")
}

pub fn update(host: Entity<StoreHost>, layer: WeakEntity<PluginLayer>, u: PluginUpdate, cx: &mut App) {
    let store = &host.read(cx).store;
    let (Some(updates), Some(registry)) = (store.plugin_updates().cloned(), store.plugins().cloned()) else { return };
    let old = registry.get(&u.id).and_then(|p| p.manifest);
    let same_origin = registry.origin_of(&u.id).as_ref() == Some(&u.origin);
    let to = u.to.clone();
    let task = host.read(cx).rt().spawn(async move { updates.prepare(&u).await });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let staged = match task.await.map_err(|e| e.to_string()).and_then(|r| r.map_err(|e| e.0)) {
            Ok(staged) => staged,
            Err(e) => {
                cx.update(|cx| update_store(&host, cx, |s| s.show_banner_with(&download_failed(&to, &e), true))).ok();
                return;
            }
        };
        let quiet = same_origin && old.is_some_and(|old| !PluginUpdates::asks_more(&old, &staged.manifest));
        if !quiet {
            if layer.update(cx, |l, cx| l.open_trust(staged.clone(), cx)).is_err() {
                registry.discard(&staged);
            }
            return;
        }
        let Ok(finished) = cx.update(|cx| finish(&host, staged, cx)) else { return };
        let Ok((outcome, staged)) = finished.await else { return };
        cx.update(|cx| match outcome {
            Ok(info) => announce(&host, &info, &staged, cx),
            Err(e) => update_store(&host, cx, |s| s.show_banner_with(&download_failed(&to, &e.0), true)),
        })
        .ok();
    })
    .detach();
}

pub struct TrustDialog {
    host: Entity<StoreHost>,
    close: Close,
    staged: Option<StagedPlugin>,
    name: String,
    updating: bool,
    error: Option<String>,
    focus: FocusHandle,
    registry: Option<mx_plugins::Plugins>,
}

impl TrustDialog {
    pub fn new(host: Entity<StoreHost>, staged: StagedPlugin, close: Close, cx: &mut Context<Self>) -> Self {
        let name = staged.manifest.name.clone();
        let updating = staged.replacing.is_some();
        let registry = host.read(cx).store.plugins().cloned();
        TrustDialog { host, close, staged: Some(staged), name, updating, error: None, focus: cx.focus_handle(), registry }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(staged) = self.staged.take() {
            discard(&self.host, &staged, cx);
        }
        (self.close)(window, cx);
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(staged) = self.staged.take() else { return };
        let task = finish(&self.host, staged, cx);
        let (host, close, handle) = (self.host.clone(), self.close.clone(), window.window_handle());
        cx.notify();
        cx.spawn(async move |me, cx: &mut AsyncApp| {
            let Ok((outcome, staged)) = task.await else { return };
            match outcome {
                Ok(info) => {
                    cx.update(|cx| announce(&host, &info, &staged, cx)).ok();
                    handle.update(cx, |_, window, cx| close(window, cx)).ok();
                }
                Err(e) => {
                    me.update(cx, |d, cx| {
                        d.error = Some(e.0);
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

impl Drop for TrustDialog {
    fn drop(&mut self) {
        if let (Some(staged), Some(r)) = (self.staged.take(), &self.registry) {
            r.discard(&staged);
        }
    }
}

impl Render for TrustDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus.is_focused(window) {
            window.focus(&self.focus);
        }
        let t = theme(cx);
        let me = cx.weak_entity();
        let body = match &self.staged {
            Some(staged) => trust_body(staged, &t),
            None => div().text_size(px(12.)).text_color(hsla(t.fg_faint)).child("Instalando…"),
        };
        let body = match &self.error {
            Some(e) => body.child(div().text_size(px(12.)).text_color(hsla(t.red)).child(e.clone())),
            None => body,
        };
        let cancel = {
            let me = me.clone();
            text_button("trust-cancel", "Cancelar", hsla(t.accent), None, move |w, cx| {
                me.update(cx, |d, cx| d.cancel(w, cx)).ok();
            })
        };
        let mut buttons = vec![cancel];
        if self.staged.is_some() {
            let me = me.clone();
            buttons.push(text_button("trust-go", trust_button(self.updating), hsla(t.on_accent()), Some(hsla(t.accent)), move |w, cx| {
                me.update(cx, |d, cx| d.confirm(w, cx)).ok();
            }));
        }
        barrier(BARRIER, move |w, cx: &mut App| {
            me.update(cx, |d, cx| d.cancel(w, cx)).ok();
        })
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .track_focus(&self.focus)
                .on_key_down(cx.listener(|d, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        d.cancel(window, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(
                    card(&t, 480.)
                        .child(div().px(px(24.)).pt(px(18.)).pb(px(10.)).child(title(&t, trust_title(&self.name, self.updating))))
                        .child(div().px(px(24.)).pb(px(8.)).child(body))
                        .child(div().px(px(16.)).py(px(12.)).flex().justify_end().gap(px(4.)).children(buttons)),
                ),
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
}
