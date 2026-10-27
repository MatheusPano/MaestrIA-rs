use gpui::{
    AnyElement, App, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use mx_plugins::rail::{NO_WINDOWS, sidebar_placeholder};
use mx_store::{Areas, Store};

use super::surface::{Surface, SurfaceEnv, TermViews};
use crate::app::host::{StoreChanged, StoreHost, update_store};
use crate::app::theme::Mx;
use crate::rfw_view::{BlocksMode, plugin_glyph};

pub const WINDOWS: &str = "Janelas";
pub const COMMANDS: &str = "Comandos";

pub struct PluginSidebar {
    host: Entity<StoreHost>,
    terms: TermViews,
    surface: Option<(String, Surface)>,
    _sub: Subscription,
}

impl PluginSidebar {
    pub fn new(host: Entity<StoreHost>, terms: TermViews, cx: &mut Context<Self>) -> Self {
        let sub = cx.subscribe(&host, |_, _, ev: &StoreChanged, cx| {
            if ev.0.areas.intersects(Areas::PLUGINS | Areas::SIDEBAR | Areas::PREFS) || !ev.0.tabs.is_empty() {
                cx.notify();
            }
        });
        PluginSidebar { host, terms, surface: None, _sub: sub }
    }
}

fn ruler(mx: &Mx, text: &'static str) -> impl IntoElement {
    div().px(px(14.)).pt(px(12.)).pb(px(4.)).text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(mx.fg_faint).child(text)
}

fn row(mx: &Mx, id: String, label: String, active: bool, act: impl Fn(&mut Window, &mut App) + 'static) -> AnyElement {
    div()
        .id(SharedString::from(id))
        .mx(px(6.))
        .px(px(8.))
        .py(px(5.))
        .rounded(px(6.))
        .text_size(px(13.))
        .text_color(if active { mx.fg } else { mx.fg_dim })
        .when(active, |d| d.bg(mx.bg_active))
        .hover(|s| s.bg(mx.bg_hover))
        .cursor_pointer()
        .truncate()
        .on_click(move |_, window, cx| act(window, cx))
        .child(label)
        .into_any_element()
}

fn windows(mx: &Mx, host: &Entity<StoreHost>, store: &Store, plugin: &str, empty: bool) -> Vec<AnyElement> {
    let focused = store.focused_tab_id();
    let open: Vec<(String, String)> = store
        .tabs()
        .iter()
        .filter(|t| !t.embedded && (store.view_plugin_of(&t.id) == Some(plugin) || t.owner.as_deref() == Some(plugin)))
        .map(|t| (t.id.clone(), store.title(t)))
        .collect();
    if open.is_empty() && !empty {
        return Vec::new();
    }
    let mut out = vec![ruler(mx, WINDOWS).into_any_element()];
    if open.is_empty() {
        out.push(div().px(px(14.)).py(px(4.)).text_size(px(12.)).text_color(mx.fg_faint).child(NO_WINDOWS).into_any_element());
    }
    for (id, title) in open {
        let host = host.clone();
        let tab = id.clone();
        let active = focused.as_deref() == Some(id.as_str()) && store.is_open(&id);
        out.push(row(mx, format!("win:{id}"), title, active, move |_, cx| update_store(&host, cx, |s| s.select(&tab))));
    }
    out
}

impl Render for PluginSidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let Some(plugin) = store.shown_plugin() else { return div().into_any_element() };
        let Some(info) = store.plugins().and_then(|r| r.get(&plugin)) else { return div().into_any_element() };
        let manifest = info.manifest.clone();
        let name = info.name().to_owned();
        let glyph = plugin_glyph(manifest.as_ref().and_then(|m| m.icon.as_deref()), Some(&info.dir), 17., mx.fg_dim);
        let header = div()
            .h(px(42.))
            .flex_none()
            .px(px(14.))
            .flex()
            .items_center()
            .gap(px(9.))
            .border_b_1()
            .border_color(mx.border)
            .child(glyph)
            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(mx.fg).child(name.clone()));
        let has_sidebar = manifest.as_ref().is_some_and(|m| m.sidebar);
        let mut col = div().size_full().flex().flex_col().child(header);
        col = col.children(windows(&mx, &self.host, store, &plugin, !has_sidebar));
        if has_sidebar {
            let view = store.sidebar_view_of(&plugin).cloned();
            match view {
                Some(view) => {
                    let tab = format!("sidebar:{plugin}");
                    if self.surface.as_ref().is_none_or(|(t, _)| *t != tab) {
                        self.surface = Some((tab.clone(), Surface::new(&tab, BlocksMode::Bare)));
                    }
                    let env = SurfaceEnv {
                        host: self.host.clone(),
                        theme: mx.theme.clone(),
                        typography: super::surface::rfw_typography(&store.prefs().typography),
                        terms: self.terms.clone(),
                        zoom: 0.0,
                    };
                    let surface = self.surface.as_mut().map(|(_, s)| s);
                    if let Some(s) = surface {
                        s.sync(&view, &env, cx);
                        col = col.child(div().flex_1().min_h_0().child(s.element()));
                    }
                }
                None => {
                    self.surface = None;
                    let crash = info.crash.clone();
                    let host = self.host.clone();
                    let id = plugin.clone();
                    col = col.child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(10.))
                            .px(px(16.))
                            .child(div().text_size(px(12.5)).text_color(mx.fg_faint).text_center().child(sidebar_placeholder(&name, crash.as_deref())))
                            .when(crash.is_some(), |d| {
                                d.child(row(&mx, "sidebar-restart".into(), "Reiniciar".into(), false, move |_, cx| {
                                    update_store(&host, cx, |s| s.restart_plugin(&id))
                                }))
                            }),
                    );
                }
            }
        } else {
            self.surface = None;
            let commands = store.plugins().map(|r| r.commands()).unwrap_or_default();
            col = col.child(ruler(&mx, COMMANDS));
            for c in commands.into_iter().filter(|c| c.plugin_id == plugin) {
                let host = self.host.clone();
                let full = c.full_id();
                let label = c.short_title(&name).to_owned();
                col = col.child(row(&mx, format!("cmd:{full}"), label, false, move |_, cx| {
                    update_store(&host, cx, |s| s.run_plugin_command(&full, None))
                }));
            }
        }
        col.into_any_element()
    }
}
