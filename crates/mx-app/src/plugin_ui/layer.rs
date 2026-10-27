use std::collections::HashMap;

use gpui::{
    AnyElement, AnyView, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, deferred, div,
};
use mx_store::{Areas, DialogAsk};

use super::form::FormDialog;
use super::log::LogDialog;
use super::pick::{InputDialog, PickDialog};
use super::settings::SettingsDialog;
use super::surface::{Surface, SurfaceEnv, TermViews};
use super::{LayerHandle, floats, modal};
use crate::app::host::{StoreChanged, StoreHost};
use crate::app::theme::Mx;

pub struct PluginLayer {
    pub(super) host: Entity<StoreHost>,
    pub(super) terms: TermViews,
    pub(super) focus: FocusHandle,
    pub(super) modal: Option<(String, Surface)>,
    pub(super) floats: HashMap<String, Surface>,
    pub(super) grab: Option<floats::Grab>,
    dialog: Option<(u64, AnyView)>,
    local: Option<AnyView>,
    local_focus: FocusHandle,
    focus_local: bool,
    escape_closes_local: bool,
    _sub: Subscription,
}

impl PluginLayer {
    pub fn new(host: Entity<StoreHost>, terms: TermViews, cx: &mut Context<Self>) -> PluginLayer {
        let sub = cx.subscribe(&host, |_, _, ev: &StoreChanged, cx| {
            if ev.0.areas.intersects(Areas::PLUGINS | Areas::PREFS | Areas::PANES) {
                cx.notify();
            }
        });
        let weak = cx.weak_entity();
        let (h, t) = (host.clone(), terms.clone());
        let sidebar = cx.new(|cx| super::sidebar::PluginSidebar::new(h, t, cx));
        cx.set_global(LayerHandle(weak, sidebar));
        PluginLayer {
            host,
            terms,
            focus: cx.focus_handle(),
            modal: None,
            floats: HashMap::new(),
            grab: None,
            dialog: None,
            local: None,
            local_focus: cx.focus_handle(),
            focus_local: false,
            escape_closes_local: false,
            _sub: sub,
        }
    }

    pub(super) fn env(&self, cx: &gpui::App) -> SurfaceEnv {
        let store = &self.host.read(cx).store;
        SurfaceEnv {
            host: self.host.clone(),
            theme: Mx::get(cx).theme.clone(),
            typography: super::surface::rfw_typography(&store.prefs().typography),
            terms: self.terms.clone(),
            zoom: 0.0,
        }
    }

    pub fn open_settings(&mut self, plugin: &str, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let close = self.closer(cx);
        let plugin = plugin.to_owned();
        self.local = Some(cx.new(|cx| SettingsDialog::new(host, plugin, close, window, cx)).into());
        self.focus_local = true;
        self.escape_closes_local = true;
        cx.notify();
    }

    pub fn open_log(&mut self, plugin: &str, _window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let close = self.closer(cx);
        let plugin = plugin.to_owned();
        self.local = Some(cx.new(|_| LogDialog::new(host, plugin, close)).into());
        self.focus_local = true;
        self.escape_closes_local = true;
        cx.notify();
    }

    pub fn covers(&self) -> bool {
        self.local.is_some() || self.dialog.is_some() || self.modal.is_some()
    }

    pub fn open_install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let close = self.closer(cx);
        self.local = Some(cx.new(|cx| super::install::InstallDialog::new(host, close, window, cx)).into());
        self.escape_closes_local = false;
        cx.notify();
    }

    pub fn open_catalog(&mut self, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let close = self.closer(cx);
        let me = cx.weak_entity();
        let from_url: super::widgets::Close = std::rc::Rc::new(move |window, cx| {
            me.update(cx, |l, cx| l.open_install(window, cx)).ok();
        });
        self.local = Some(cx.new(|cx| super::catalog::CatalogDialog::new(host, close, from_url, cx)).into());
        self.escape_closes_local = false;
        cx.notify();
    }

    pub fn open_trust(&mut self, staged: mx_plugins::StagedPlugin, cx: &mut Context<Self>) {
        let host = self.host.clone();
        let close = self.closer(cx);
        self.local = Some(cx.new(|cx| super::trust::TrustDialog::new(host, staged, close, cx)).into());
        self.escape_closes_local = false;
        cx.notify();
    }

    fn closer(&self, cx: &mut Context<Self>) -> super::widgets::Close {
        let me = cx.weak_entity();
        std::rc::Rc::new(move |_, cx| {
            me.update(cx, |l, cx| {
                l.local = None;
                cx.notify();
            })
            .ok();
        })
    }

    fn sync_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = &self.host.read(cx).store;
        let Some(d) = store.plugin_dialog() else {
            self.dialog = None;
            return;
        };
        if self.dialog.as_ref().is_some_and(|(id, _)| *id == d.id) {
            return;
        }
        let (id, ask, name) = (d.id, d.ask.clone(), d.plugin.name().to_owned());
        let host = self.host.clone();
        let view: AnyView = match ask {
            DialogAsk::Pick(req) => cx.new(|cx| PickDialog::new(host, req, window, cx)).into(),
            DialogAsk::Input(req) => cx.new(|cx| InputDialog::new(host, req, window, cx)).into(),
            DialogAsk::Form(form) => cx.new(|cx| FormDialog::new(host, form, name, window, cx)).into(),
        };
        self.dialog = Some((id, view));
    }
}

impl Render for PluginLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_dialog(window, cx);
        let mut layers: Vec<AnyElement> = floats::render(self, window, cx);
        if let Some(m) = modal::render(self, window, cx) {
            layers.push(m);
        }
        let surfaces = self.floats.values().chain(self.modal.as_ref().map(|(_, s)| s));
        let menus: Vec<AnyElement> = surfaces.filter_map(|s| s.menu_overlay(cx)).collect();
        layers.extend(menus.into_iter().map(|m| deferred(m).with_priority(2).into_any_element()));
        if let Some((_, d)) = &self.dialog {
            layers.push(full(window, d.clone().into_any_element()).with_priority(3).into_any_element());
        }
        if self.focus_local && self.local.is_some() {
            self.focus_local = false;
            window.focus(&self.local_focus);
        }
        if let Some(l) = &self.local {
            let keyed = div()
                .size_full()
                .track_focus(&self.local_focus)
                .capture_key_down(cx.listener(|layer, ev: &gpui::KeyDownEvent, _, cx| {
                    if layer.escape_closes_local && ev.keystroke.key == "escape" {
                        layer.local = None;
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .child(l.clone());
            layers.push(full(window, keyed.into_any_element()).with_priority(4).into_any_element());
        }
        div().children(layers)
    }
}

pub(super) fn full(window: &Window, child: AnyElement) -> gpui::Deferred {
    let size = window.viewport_size();
    deferred(
        gpui::anchored()
            .position(gpui::point(gpui::px(0.), gpui::px(0.)))
            .child(gpui::div().w(size.width).h(size.height).relative().child(child)),
    )
}

