use gpui::{
    AnyView, App, AppContext, ClipboardItem, Context, DragMoveEvent, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, deferred, div, prelude::FluentBuilder, px,
};
use mx_store::{Areas, Changes, Effect, Store};

use std::rc::Rc;

use crate::app::host::{StoreChanged, StoreHost, update_store};
use crate::app::theme::{GAP, Mx};
use crate::app::ui::{self, Ui};
use crate::app::{dialogs, settings};
use crate::chrome::settings::Section;
use crate::keymap::{self, Outcome, UiRequest};
use crate::chrome::notices::Banner;
use crate::chrome::widgets::tip_gate;
use crate::panes::PaneArea;
use crate::rail::{self, Rail};
use crate::sidebar::{self, Sidebar};
use crate::status_bar::{self, SessionsView, StatusBar};
use crate::chrome::anim::slots;

const ROOT_CONTEXT: &str = "MaestriaRoot";
const NOTICES_ABOVE_PLUGIN_LAYERS: usize = 5;

#[derive(Debug, Clone, Copy)]
struct SidebarGrip;

pub struct Root {
    host: Entity<StoreHost>,
    ui: Entity<Ui>,
    rail: Entity<Rail>,
    sidebar: Entity<Sidebar>,
    status_bar: Entity<StatusBar>,
    panes: Entity<PaneArea>,
    sessions: Entity<SessionsView>,
    sessions_open: bool,
    theme: String,
    focus: FocusHandle,
}

pub fn fit_sidebar(width: f32, window_width: f32) -> f32 {
    let max = sidebar::MIN_WIDTH.max(window_width - 280.);
    width.clamp(sidebar::MIN_WIDTH, max)
}

pub fn routes(c: &Changes) -> (bool, bool, bool, bool, bool) {
    let a = c.areas;
    let tabs = !c.tabs.is_empty();
    let sidebar = tabs
        || a.intersects(Areas::SIDEBAR | Areas::SEARCH | Areas::GROUPS | Areas::LAUNCHERS | Areas::PREFS);
    let panes = tabs || a.intersects(Areas::PANES | Areas::PREFS);
    let status = tabs || a.intersects(Areas::NOTICES | Areas::PREFS);
    let rail = tabs || a.intersects(Areas::SIDEBAR | Areas::PREFS);
    let root = a.intersects(Areas::PREFS | Areas::BANNER | Areas::NOTICES);
    (sidebar, panes, status, rail, root)
}

impl Root {
    pub fn new(host: Entity<StoreHost>, window: &mut Window, cx: &mut Context<Self>) -> Root {
        let rail = cx.new(|_| Rail::new(host.clone()));
        let sidebar = cx.new(|cx| Sidebar::new(host.clone(), window, cx));
        let status_bar = cx.new(|_| StatusBar::new(host.clone()));
        let panes = cx.new(|_| PaneArea::new(host.clone()));
        let sessions = cx.new(|_| SessionsView::new(host.clone()));
        cx.subscribe_in(&host, window, Self::on_store).detach();
        let ui = ui::get(cx);
        cx.observe(&ui, |this, _, cx| {
            this.panes.update(cx, |_, cx| cx.notify());
            cx.notify();
        })
        .detach();
        cx.observe_window_activation(window, |this, window, cx| {
            let active = window.is_window_active();
            update_store(&this.host, cx, |s| s.set_window_active(active));
        })
        .detach();
        let theme = host.read(cx).store.prefs().theme.clone();
        let resolved = Mx::resolve(&host.read(cx).store);
        Mx::install(resolved, cx);
        Root {
            host,
            ui,
            rail,
            sidebar,
            status_bar,
            panes,
            sessions,
            sessions_open: false,
            theme,
            focus: cx.focus_handle(),
        }
    }

    fn on_store(&mut self, _: &Entity<StoreHost>, ev: &StoreChanged, window: &mut Window, cx: &mut Context<Self>) {
        for effect in &ev.1 {
            match effect {
                Effect::Clipboard(text) => cx.write_to_clipboard(ClipboardItem::new_string(text.clone())),
                Effect::Paste { tab_id, text } => {
                    if let Some(session) = self.host.read(cx).terms.get(tab_id) {
                        session.paste(text);
                    }
                }
            }
        }
        let theme = self.host.read(cx).store.prefs().theme.clone();
        if theme != self.theme {
            let resolved = Mx::resolve(&self.host.read(cx).store);
            Mx::install(resolved, cx);
            let palette = Mx::get(cx).theme.terminal();
            self.host.read(cx).terms.set_palette(palette.clone());
            self.panes.update(cx, |p, cx| p.set_palette(&palette, cx));
            self.theme = theme;
            window.refresh();
            return;
        }
        let (sidebar, panes, status, rail, root) = routes(&ev.0);
        if sidebar {
            self.sidebar.update(cx, |_, cx| cx.notify());
        }
        if panes {
            self.panes.update(cx, |_, cx| cx.notify());
        }
        if status {
            self.status_bar.update(cx, |_, cx| cx.notify());
        }
        if rail {
            self.rail.update(cx, |_, cx| cx.notify());
        }
        let open = self.host.read(cx).store.bell().sessions_open;
        let opened = open && !self.sessions_open;
        self.sessions_open = open;
        if open && (opened || status) {
            self.sessions.update(cx, |_, cx| cx.notify());
        }
        if root {
            cx.notify();
        }
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        if k.key == "escape" && !(m.control || m.shift || m.alt || m.platform) {
            if ui::escape(window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if self.ui.read(cx).top().is_some() {
            return;
        }
        let Outcome::Ui(request) = keymap::on_key_down(&self.host, ev, window, cx) else { return };
        match request {
            UiRequest::Settings => settings::open(Section::Appearance, window, cx),
            UiRequest::FocusSearch => {
                update_store(&self.host, cx, |s| s.set_sidebar_hidden(false));
                let sidebar = self.sidebar.clone();
                window.defer(cx, move |window, cx| sidebar.read(cx).focus_search(window));
            }
            UiRequest::NewTask { folder } => dialogs::new_task(&folder, None, window, cx),
            UiRequest::Rename { tab_id, title } => dialogs::prompt(
                "Renomear painel",
                "Título",
                &title,
                move |text, cx| ui::act(cx, |s| s.rename_tab(&tab_id, &text)),
                window,
                cx,
            ),
        }
    }

    fn grip(&self, mx: &Mx) -> impl IntoElement {
        let host = self.host.clone();
        div()
            .id("sidebar-grip")
            .group("sidebar-grip")
            .w(px(GAP))
            .h_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_col_resize()
            .on_drag(SidebarGrip, |_, _, _, cx| cx.new(|_| Blank))
            .on_click(move |ev, _, cx: &mut App| {
                if ev.click_count() == 2 {
                    update_store(&host, cx, |s| s.set_sidebar_width(sidebar::DEFAULT_WIDTH as f64));
                }
            })
            .child(
                div()
                    .w(px(3.))
                    .h(px(0.))
                    .rounded(px(2.))
                    .bg(mx.fg_faint)
                    .group_hover("sidebar-grip", |s| s.h(px(46.))),
            )
    }
}

struct Blank;

impl Render for Blank {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        window.on_next_frame(move |window, _| {
            if !window.context_stack().iter().any(|c| c.contains(ROOT_CONTEXT)) {
                window.focus(&focus);
            }
        });
        crate::chrome::dialog::frame::forget_escape(cx);
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let prefs = store.prefs();
        let top = prefs.status_bar_top;
        let hidden = prefs.sidebar_hidden;
        let width = fit_sidebar(prefs.sidebar_width as f32, f32::from(window.viewport_size().width));
        let sessions_open = store.bell().sessions_open;
        let banner_text = store.banner().map(str::to_owned);

        let rail = AnyView::from(self.rail.clone())
            .cached(StyleRefinement::default().w(px(rail::WIDTH + GAP)).h_full().flex_none());
        let sidebar = AnyView::from(self.sidebar.clone())
            .cached(StyleRefinement::default().w(px(width)).h_full().flex_none());
        let panes = AnyView::from(self.panes.clone()).cached(StyleRefinement::default().size_full());
        let status = AnyView::from(self.status_bar.clone()).cached(
            StyleRefinement::default().w_full().h(px(status_bar::HEIGHT)).flex_none(),
        );
        let host = self.host.clone();
        let mut owners = vec![self.panes.entity_id()];
        if !hidden {
            owners.push(self.sidebar.entity_id());
        }

        let stage = div()
            .flex_1()
            .relative()
            .child(panes)
            .child(slots::layer(owners))
            .child(deferred(self.notices(banner_text, top, cx)).with_priority(NOTICES_ABOVE_PLUGIN_LAYERS));

        let body = div()
            .size_full()
            .pl(px(GAP))
            .pr(px(GAP))
            .pt(px(if top { 0. } else { GAP }))
            .pb(px(if top { GAP } else { 0. }))
            .flex()
            .flex_row()
            .child(rail)
            .when(!hidden, |d| d.child(sidebar).child(self.grip(&mx)))
            .child(stage);

        let ui = self.ui.read(cx);
        let layers = ui.views();
        let confetti = ui.confetti.clone();
        div()
            .id("root")
            .key_context(ROOT_CONTEXT)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(mx.canvas)
            .text_color(mx.fg)
            .capture_key_down(cx.listener(Self::on_key))
            .on_drag_move(move |ev: &DragMoveEvent<SidebarGrip>, window, cx: &mut App| {
                let x = f32::from(ev.event.position.x) - status_bar::INSET;
                let w = fit_sidebar(x, f32::from(window.viewport_size().width));
                update_store(&host, cx, |s| s.set_sidebar_width(w as f64));
            })
            .when(top, |d| d.child(status.clone()))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(body)
                    .when(sessions_open, |d| {
                        let view = AnyView::from(self.sessions.clone())
                            .cached(StyleRefinement::default().absolute().inset_0());
                        d.child(view).child(slots::layer(vec![self.sessions.entity_id()]))
                    }),
            )
            .when(!top, |d| d.child(status))
            .children(layers.into_iter().map(|v| div().absolute().inset_0().child(v)))
            .child(confetti)
            .child(tip_gate::watch_clicks())
            .child(div().id("root-focus").track_focus(&self.focus).absolute().size_0())
    }
}

impl Root {
    fn notices(&self, banner_text: Option<String>, top: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let ui = self.ui.read(cx);
        if let Some(center) = ui.center.clone() {
            let pos = div().absolute().right(px(8.));
            let pos = if top { pos.top(px(8.)) } else { pos.bottom(px(8.)) };
            return pos.child(center).into_any_element();
        }
        let banner = banner_text.map(|text| {
            Banner::new(text, Rc::new(|_, cx: &mut App| ui::act(cx, Store::clear_banner))).on_top(top)
        });
        let col = div().absolute().left(px(8.)).right(px(8.)).flex().flex_col().items_end().gap(px(8.));
        let col = if top { col.top(px(8.)) } else { col.bottom(px(8.)) };
        let toasts = ui.toasts.clone();
        match (top, banner) {
            (true, Some(b)) => col.child(b).child(toasts),
            (false, Some(b)) => col.child(toasts).child(b),
            (_, None) => col.child(toasts),
        }
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidebar_never_goes_below_248_nor_eats_the_last_280_px() {
        assert_eq!(fit_sidebar(100., 1280.), 248.);
        assert_eq!(fit_sidebar(352., 1280.), 352.);
        assert_eq!(fit_sidebar(1200., 1280.), 1000.);
        assert_eq!(fit_sidebar(400., 300.), 248.);
    }

    #[test]
    fn a_banner_change_repaints_only_the_root() {
        let c = Changes {
            areas: Areas::BANNER,
            tabs: vec![],
        };
        assert_eq!(routes(&c), (false, false, false, false, true));
    }

    #[test]
    fn a_pane_change_leaves_the_sidebar_alone() {
        let c = Changes {
            areas: Areas::PANES,
            tabs: vec![],
        };
        assert_eq!(routes(&c), (false, true, false, false, false));
    }
}
