use gpui::{
    AnyView, App, AppContext, ClipboardItem, Context, DragMoveEvent, Entity, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use mx_store::{Areas, Changes, Effect, Store};

use crate::app::banner::banner;
use crate::app::host::{StoreChanged, StoreHost, update_store};
use crate::app::theme::{GAP, Mx};
use crate::panes::PaneArea;
use crate::rail::{self, Rail};
use crate::sidebar::{self, Sidebar};
use crate::status_bar::{self, SessionList, StatusBar};

#[derive(Debug, Clone, Copy)]
struct SidebarGrip;

pub struct Root {
    host: Entity<StoreHost>,
    rail: Entity<Rail>,
    sidebar: Entity<Sidebar>,
    status_bar: Entity<StatusBar>,
    panes: Entity<PaneArea>,
    theme: String,
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
        cx.subscribe_in(&host, window, Self::on_store).detach();
        cx.observe_window_activation(window, |this, window, cx| {
            let active = window.is_window_active();
            update_store(&this.host, cx, |s| s.set_window_active(active));
        })
        .detach();
        let theme = host.read(cx).store.prefs().theme.clone();
        Mx::install(&theme, cx);
        Root {
            host,
            rail,
            sidebar,
            status_bar,
            panes,
            theme,
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
            Mx::install(&theme, cx);
            let palette = Mx::get(cx).theme.terminal();
            self.host.read(cx).terms.set_palette(palette);
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
        let sessions_open = self.host.read(cx).store.bell().sessions_open;
        if root || (sessions_open && !ev.0.tabs.is_empty()) {
            cx.notify();
        }
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        if !(m.control && m.shift) || m.alt {
            return;
        }
        match k.key.as_str() {
            "b" => update_store(&self.host, cx, Store::toggle_sidebar),
            "f" => {
                update_store(&self.host, cx, |s| s.set_sidebar_hidden(false));
                self.sidebar.read(cx).focus_search(window);
            }
            _ => return,
        }
        cx.stop_propagation();
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
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let prefs = store.prefs();
        let top = prefs.status_bar_top;
        let hidden = prefs.sidebar_hidden;
        let width = fit_sidebar(prefs.sidebar_width as f32, f32::from(window.viewport_size().width));
        let sessions = store.bell().sessions_open.then(|| SessionList::of(store));
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
        let close_host = self.host.clone();

        let mut stage = div()
            .flex_1()
            .relative()
            .child(panes);
        if let Some(text) = banner_text {
            stage = stage.child(banner(&mx, &self.host, text, top));
        }

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

        div()
            .id("root")
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
                    .when_some(sessions, |d, list| {
                        let pos = div().absolute().left(px(status_bar::INSET));
                        let pos = if top { pos.top(px(8.)) } else { pos.bottom(px(8.)) };
                        d.child(
                            pos.on_mouse_down_out(move |ev, window, cx: &mut App| {
                                let y = f32::from(ev.position.y);
                                let h = f32::from(window.viewport_size().height);
                                let on_bar = if top { y < status_bar::HEIGHT } else { y > h - status_bar::HEIGHT };
                                if !on_bar {
                                    update_store(&close_host, cx, Store::close_sessions)
                                }
                            })
                            .child(list.render(&mx, &self.host)),
                        )
                    }),
            )
            .when(!top, |d| d.child(status))
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
