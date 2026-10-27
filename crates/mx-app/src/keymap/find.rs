use gpui::{App, Entity, Window};

use crate::app::host::{StoreHost, update_store};
use crate::plugin_ui::surface::TermViews;
use crate::terminal_view::TerminalView;

pub const NO_TERMINAL: &str = "Nenhum terminal em foco pra buscar";

pub fn open(host: &Entity<StoreHost>, views: &TermViews, tab_id: Option<&str>, window: &mut Window, cx: &mut App) {
    let target = {
        let views = views.borrow();
        views
            .values()
            .find(|v| v.read(cx).owns_focus(window, cx))
            .or_else(|| tab_id.and_then(|id| views.get(id)))
            .cloned()
    };
    match target {
        Some(view) => open_in(&view, window, cx),
        None => update_store(host, cx, |s| s.show_banner(NO_TERMINAL)),
    }
}

fn open_in(view: &Entity<TerminalView>, window: &mut Window, cx: &mut App) {
    view.update(cx, |v, cx| v.open_find(window, cx));
}
