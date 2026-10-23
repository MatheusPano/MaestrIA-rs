use std::rc::Rc;

use gpui::{App, Window};
use mx_core::text::short_ago;
use mx_store::{NoticeEntry, Store};

use crate::app::ui;
use crate::chrome::notices::{NoticeCenterProps, NoticeView, RowHandlers};
use crate::status_bar;

pub fn view_of(store: &Store, n: &NoticeEntry, now: &mx_core::text::Time) -> NoticeView {
    let tab = store.tab(&n.notice.tab_id);
    let folder = tab
        .map(|t| store.folder_of(t))
        .filter(|f| !f.is_loose)
        .map(|f| f.name.clone());
    NoticeView {
        id: n.id,
        kind: n.notice.kind,
        title: tab.map_or_else(|| n.notice.title.clone(), |t| store.title(t)),
        detail: n.notice.detail.clone(),
        read: n.notice.read,
        gone: tab.is_none(),
        folder,
        ago: short_ago(&n.notice.at, now),
    }
}

pub fn toast_views(store: &Store) -> Vec<NoticeView> {
    let now = chrono::Local::now();
    store.toasts().into_iter().map(|n| view_of(store, n, &now)).collect()
}

pub fn list_views(store: &Store) -> Vec<NoticeView> {
    let now = chrono::Local::now();
    store.notices().iter().map(|n| view_of(store, n, &now)).collect()
}

pub fn toast_handlers() -> RowHandlers {
    RowHandlers {
        on_open: Rc::new(|id, _, cx| ui::act(cx, |s| s.open_notice(id))),
        on_close: Rc::new(|id, _, cx| ui::act(cx, |s| s.dismiss_toast(id))),
        on_hover: Some(Rc::new(|(id, on), _, cx| ui::act(cx, |s| s.hold_toast(id, on)))),
    }
}

fn on_bar(window: &Window, cx: &App) -> bool {
    let top = ui::store(cx).prefs().status_bar_top;
    let y = f32::from(window.mouse_position().y);
    let h = f32::from(window.viewport_size().height);
    if top { y < status_bar::HEIGHT } else { y > h - status_bar::HEIGHT }
}

pub fn center_props() -> NoticeCenterProps {
    NoticeCenterProps {
        on_toggle_dnd: Rc::new(|_, cx| ui::act(cx, Store::toggle_do_not_disturb)),
        on_read_all: Rc::new(|_, cx| ui::act(cx, Store::read_all_notices)),
        on_clear_all: Rc::new(|cx| ui::act(cx, Store::clear_notices)),
        on_close: Rc::new(|window, cx| {
            if !on_bar(window, cx) {
                ui::act(cx, Store::close_notices)
            }
        }),
        on_open: Rc::new(|id, _, cx| ui::act(cx, |s| s.open_notice(id))),
        on_dismiss: Rc::new(|id, _, cx| ui::act(cx, |s| s.dismiss_notice(id))),
    }
}
