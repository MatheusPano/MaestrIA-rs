use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use mx_plugins::catalog::{CatalogEntry, PluginCatalog, PluginUpdates, fetch_icon};
use mx_plugins::{Permission, StagedPlugin};
use mx_store::Areas;
use mx_theme::Theme;

use super::kind_tag::kind_tag;
use super::trust::TrustDialog;
use super::widgets::{Close, barrier, card, text_button, title};
use crate::app::host::{StoreChanged, StoreHost};
use crate::chrome::anim::Spinner;
use crate::chrome::dialog::frame::BARRIER;
use crate::chrome::style::{MONO, hsla, theme};

pub const CATALOG_TITLE: &str = "Catálogo de plugins";
pub const FROM_URL: &str = "De uma URL, .zip ou pasta…";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryAction {
    Busy,
    Dev,
    TooNew,
    Installed,
    Install,
    Update,
}

pub fn entry_action(e: &CatalogEntry, have: Option<(&str, bool)>, busy: bool) -> EntryAction {
    if busy {
        return EntryAction::Busy;
    }
    if have.is_some_and(|(_, linked)| linked) {
        return EntryAction::Dev;
    }
    if !e.fits() {
        return EntryAction::TooNew;
    }
    match have {
        Some((v, _)) if mx_core::version::compare_versions(&e.version, v).is_le() => EntryAction::Installed,
        Some(_) => EntryAction::Update,
        None => EntryAction::Install,
    }
}

pub fn version_text(new: &str, have: Option<&str>) -> String {
    match have {
        Some(old) if old != new => format!("{old} → {new}"),
        _ => new.to_owned(),
    }
}

pub fn asks_text(permissions: &[String]) -> Option<String> {
    if permissions.is_empty() {
        return None;
    }
    let labels: Vec<&str> = permissions.iter().map(|p| match Permission::by_id(p) {
        Some(known) => known.label(),
        None => p.as_str(),
    }).collect();
    Some(format!("Pode {}", labels.join("; ")))
}

enum Icon {
    Loading,
    Missing,
    Ready(PathBuf),
}

pub struct CatalogDialog {
    host: Entity<StoreHost>,
    close: Close,
    open_install: Close,
    focus: FocusHandle,
    loading: bool,
    error: Option<String>,
    busy: BTreeSet<String>,
    errors: HashMap<String, String>,
    icons: HashMap<String, Icon>,
    trust: Option<Entity<TrustDialog>>,
    _sub: Subscription,
}

fn icon_dir() -> PathBuf {
    std::env::temp_dir().join(format!("maestria-catalog-icons-{}", std::process::id()))
}

impl CatalogDialog {
    pub fn new(host: Entity<StoreHost>, close: Close, open_install: Close, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&host, |_, _, ev: &StoreChanged, cx| {
            if ev.0.areas.intersects(Areas::PLUGINS) {
                cx.notify();
            }
        });
        let mut d = CatalogDialog {
            host,
            close,
            open_install,
            focus: cx.focus_handle(),
            loading: false,
            error: None,
            busy: BTreeSet::new(),
            errors: HashMap::new(),
            icons: HashMap::new(),
            trust: None,
            _sub: sub,
        };
        d.reload(cx);
        d
    }

    fn updates(&self, cx: &App) -> Option<PluginUpdates> {
        self.host.read(cx).store.plugin_updates().cloned()
    }

    fn url(&self, cx: &App) -> Option<String> {
        self.updates(cx).and_then(|u| u.catalogs().first().cloned())
    }

    fn catalog(&self, cx: &App) -> Option<PluginCatalog> {
        let u = self.updates(cx)?;
        u.loaded(u.catalogs().first()?)
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let (Some(u), Some(url)) = (self.updates(cx), self.url(cx)) else {
            self.error = Some("Os plugins não estão carregados".into());
            return;
        };
        self.loading = true;
        self.error = None;
        let task = self.host.read(cx).rt().spawn(async move { u.load(&url).await });
        cx.spawn(async move |me, cx| {
            let out = task.await.unwrap_or_else(|e| Err(e.to_string()));
            me.update(cx, |d, cx| {
                d.loading = false;
                d.error = out.err().map(|e| format!("Não deu pra ler o catálogo: {e}"));
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn want_icon(&mut self, url: &str, cx: &mut Context<Self>) {
        if self.icons.contains_key(url) {
            return;
        }
        self.icons.insert(url.to_owned(), Icon::Loading);
        let key = url.to_owned();
        let fetch = key.clone();
        let task = self.host.read(cx).rt().spawn(async move {
            let bytes = fetch_icon(&fetch).await?;
            let dir = icon_dir();
            std::fs::create_dir_all(&dir).ok()?;
            let file = dir.join(format!("{:016x}.svg", seahash(&fetch)));
            std::fs::write(&file, bytes).ok()?;
            Some(file)
        });
        cx.spawn(async move |me, cx| {
            let file = task.await.ok().flatten();
            me.update(cx, |d, cx| {
                d.icons.insert(key, file.map_or(Icon::Missing, Icon::Ready));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn install(&mut self, e: CatalogEntry, cx: &mut Context<Self>) {
        let (Some(u), Some(url)) = (self.updates(cx), self.url(cx)) else { return };
        let registry = self.host.read(cx).store.plugins().cloned();
        let id = e.id.clone();
        self.busy.insert(id.clone());
        self.errors.remove(&id);
        let task = self.host.read(cx).rt().spawn(async move { u.stage_entry(&e, &url).await });
        cx.spawn(async move |me, cx| {
            let out = task.await.map_err(|e| e.to_string()).and_then(|r| r.map_err(|e| e.0));
            if me.update(cx, |_, _| ()).is_err() {
                if let (Ok(staged), Some(r)) = (&out, registry) {
                    r.discard(staged);
                }
                return;
            }
            me.update(cx, |d, cx| {
                d.busy.remove(&id);
                match out {
                    Ok(staged) => d.ask_trust(staged, cx),
                    Err(e) => {
                        d.errors.insert(id, e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn ask_trust(&mut self, staged: StagedPlugin, cx: &mut Context<Self>) {
        let me = cx.weak_entity();
        let close: Close = std::rc::Rc::new(move |_, cx| {
            me.update(cx, |d, cx| {
                d.trust = None;
                cx.notify();
            })
            .ok();
        });
        let host = self.host.clone();
        self.trust = Some(cx.new(|cx| TrustDialog::new(host, staged, close, cx)));
    }

    fn entry(&mut self, e: &CatalogEntry, updates: &PluginUpdates, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let info = self.host.read(cx).store.plugins().and_then(|r| r.get(&e.id));
        let have = info.as_ref().map(|p| (p.manifest.as_ref().map(|m| m.version.clone()).unwrap_or_default(), p.linked));
        let busy = self.busy.contains(&e.id) || updates.is_busy(&e.id);
        let action = entry_action(e, have.as_ref().map(|(v, l)| (v.as_str(), *l)), busy);
        let glyph = match e.icon.as_deref() {
            Some(url) => {
                self.want_icon(url, cx);
                match self.icons.get(url) {
                    Some(Icon::Ready(file)) => {
                        let name = file.file_name().map(|n| n.to_string_lossy().into_owned());
                        crate::rfw_view::plugin_glyph(name.as_deref(), file.parent(), 17., hsla(t.fg_dim))
                    }
                    _ => crate::rfw_view::plugin_glyph(None, None, 17., hsla(t.fg_dim)),
                }
            }
            None => crate::rfw_view::plugin_glyph(None, None, 17., hsla(t.fg_dim)),
        };
        let mut head = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(div().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(t.fg)).child(e.name.clone()))
            .child(div().font_family(MONO).text_size(px(11.)).text_color(hsla(t.fg_faint)).child(version_text(&e.version, have.as_ref().map(|(v, _)| v.as_str()))))
            .children(e.kinds.iter().map(|k| kind_tag(*k, t)));
        if e.git.is_some() {
            head = head.child(div().text_size(px(11.)).text_color(hsla(t.fg_faint)).child("De fora"));
        }
        let mut body = div().flex().flex_col().gap(px(3.)).pl(px(28.)).text_size(px(11.5));
        if !e.description.is_empty() {
            body = body.child(div().text_color(hsla(t.fg_dim)).line_clamp(2).child(e.description.clone()));
        }
        if let Some(asks) = asks_text(&e.permissions) {
            body = body.child(div().text_color(hsla(t.yellow)).line_clamp(2).child(asks));
        }
        if let Some(err) = self.errors.get(&e.id) {
            body = body.child(div().text_color(hsla(t.red)).child(err.clone()));
        }
        let trailing: AnyElement = match action {
            EntryAction::Busy => Spinner::new(16., t.accent).into_any_element(),
            EntryAction::Dev => div().text_size(px(11.5)).text_color(hsla(t.purple)).child("Desenvolvimento").into_any_element(),
            EntryAction::TooNew => div()
                .text_size(px(11.)).text_right().text_color(hsla(t.fg_dim))
                .child(div().child("Pede uma MaestrIA")).child(div().child("mais nova"))
                .into_any_element(),
            EntryAction::Installed => div().text_size(px(11.5)).text_color(hsla(t.fg_faint)).child("Instalado").into_any_element(),
            EntryAction::Install | EntryAction::Update => {
                let label = if action == EntryAction::Install { "Instalar" } else { "Atualizar" };
                let me = cx.weak_entity();
                let entry = e.clone();
                text_button(format!("catalog-{}", e.id), label, hsla(t.accent), Some(hsla(t.accent).opacity(0.14)), move |_, cx| {
                    let entry = entry.clone();
                    me.update(cx, |d, cx| d.install(entry, cx)).ok();
                })
            }
        };
        div()
            .flex()
            .items_start()
            .gap(px(10.))
            .p(px(12.))
            .rounded(px(9.))
            .border_1()
            .border_color(hsla(t.border))
            .bg(hsla(t.bg_hover).opacity(0.4))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(div().flex().items_center().gap(px(11.)).child(glyph).child(head))
                    .child(body),
            )
            .child(div().flex_none().child(trailing))
            .into_any_element()
    }
}

fn seahash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

impl Render for CatalogDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.trust.is_none() && !self.focus.contains_focused(window, cx) {
            window.focus(&self.focus);
        }
        let t = theme(cx);
        let catalog = self.catalog(cx);
        let updates = self.updates(cx);
        let content: AnyElement = match (&catalog, &updates) {
            (Some(c), Some(u)) => {
                let rows: Vec<AnyElement> = c.plugins.clone().iter().map(|e| self.entry(e, u, &t, cx)).collect();
                div().id("catalog-list").size_full().overflow_y_scroll().flex().flex_col().gap(px(8.)).children(rows).into_any_element()
            }
            _ => match self.error.clone() {
                Some(e) => {
                    let me = cx.weak_entity();
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(10.))
                        .child(div().text_size(px(12.5)).text_center().text_color(hsla(t.red)).child(e))
                        .child(text_button("catalog-retry", "Tentar de novo", hsla(t.accent), None, move |_, cx| {
                            me.update(cx, |d, cx| d.reload(cx)).ok();
                        }))
                        .into_any_element()
                }
                None => div().into_any_element(),
            },
        };
        let mut head = div().flex().items_center().gap(px(10.)).child(title(&t, CATALOG_TITLE));
        if self.loading {
            head = head.child(Spinner::new(14., t.accent));
        }
        let (close, from_url) = (self.close.clone(), self.open_install.clone());
        let buttons = vec![
            text_button("catalog-from-url", FROM_URL, hsla(t.accent), None, move |w, cx| from_url(w, cx)),
            {
                let close = close.clone();
                text_button("catalog-close", "Fechar", hsla(t.accent), None, move |w, cx| close(w, cx))
            },
        ];
        let dialog = barrier(BARRIER, move |w, cx: &mut App| close(w, cx)).flex().items_center().justify_center().child(
            div()
                .track_focus(&self.focus)
                .on_key_down(cx.listener(|d, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" && d.trust.is_none() {
                        (d.close)(window, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(
                    card(&t, 608.)
                        .child(div().px(px(24.)).pt(px(18.)).pb(px(10.)).child(head))
                        .child(div().px(px(24.)).h(px(460.)).child(content))
                        .child(div().px(px(16.)).py(px(12.)).flex().justify_end().gap(px(4.)).children(buttons)),
                ),
        );
        let mut layers = div().size_full().relative().child(dialog);
        if let Some(trust) = &self.trust {
            layers = layers.child(div().absolute().inset_0().child(trust.clone()));
        }
        layers.id(SharedString::from("plugin-catalog"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(json: serde_json::Value) -> CatalogEntry {
        CatalogEntry::from_json(&json).unwrap()
    }

    fn zip(version: &str) -> CatalogEntry {
        entry(serde_json::json!({"id": "mx.git", "version": version, "url": "https://x/z.zip", "sha256": "ab"}))
    }

    #[test]
    fn the_entry_action_follows_the_dart_order() {
        let e = zip("1.2.0");
        assert_eq!(entry_action(&e, None, true), EntryAction::Busy);
        assert_eq!(entry_action(&e, Some(("9.0.0", true)), false), EntryAction::Dev);
        assert_eq!(entry_action(&e, None, false), EntryAction::Install);
        assert_eq!(entry_action(&e, Some(("1.1.0", false)), false), EntryAction::Update);
        assert_eq!(entry_action(&e, Some(("1.2.0", false)), false), EntryAction::Installed);
        assert_eq!(entry_action(&e, Some(("2.0.0", false)), false), EntryAction::Installed);
        let far = entry(serde_json::json!({"id": "a", "version": "1", "git": "https://x/a", "maestria": 99}));
        assert_eq!(entry_action(&far, None, false), EntryAction::TooNew);
    }

    #[test]
    fn the_version_shows_the_jump_only_when_installed_and_different() {
        assert_eq!(version_text("1.2.0", None), "1.2.0");
        assert_eq!(version_text("1.2.0", Some("1.2.0")), "1.2.0");
        assert_eq!(version_text("1.2.0", Some("1.1.0")), "1.1.0 → 1.2.0");
    }

    #[test]
    fn permissions_read_as_labels_and_unknown_ids_stay_raw() {
        assert_eq!(asks_text(&[]), None);
        assert_eq!(
            asks_text(&["notifications".into(), "x.y".into()]).unwrap(),
            "Pode mandar notificações do sistema; x.y"
        );
    }
}
