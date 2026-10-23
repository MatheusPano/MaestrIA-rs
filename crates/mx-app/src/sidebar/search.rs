use gpui::{
    App, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::app::host::update_store;
use crate::app::icons::{Icon, icon};
use crate::app::keys;
use crate::keymap::Action;
use crate::app::menus::{self, Target};
use crate::app::theme::Mx;
use crate::sidebar::Sidebar;
use crate::sidebar::widgets::mini_button;

pub fn filter_tooltip(active: usize) -> String {
    match active {
        0 => "Filtrar por pasta, feature/hotfix ou estado".into(),
        1 => "1 filtro — clique pra mexer".into(),
        n => format!("{n} filtros — clique pra mexer"),
    }
}

pub fn edited(query: &str, key: &str, key_char: Option<&str>) -> Option<String> {
    match key {
        "backspace" => {
            let mut q = query.to_owned();
            q.pop()?;
            Some(q)
        }
        _ => {
            let ch = key_char.filter(|c| !c.chars().any(char::is_control))?;
            Some(format!("{query}{ch}"))
        }
    }
}

impl Sidebar {
    pub(super) fn on_search_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        if k.modifiers.control || k.modifiers.platform || k.modifiers.alt {
            return;
        }
        let store = &self.host.read(cx).store;
        if k.key == "escape" {
            if store.filtering() {
                update_store(&self.host, cx, |s| s.clear_search());
            } else {
                window.blur();
            }
            cx.stop_propagation();
            return;
        }
        let query = store.search().query.clone();
        if let Some(q) = edited(&query, &k.key, k.key_char.as_deref()) {
            update_store(&self.host, cx, |s| s.set_query(&q));
            cx.stop_propagation();
        }
    }

    pub(super) fn search_field(&self, mx: &Mx, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let store = &self.host.read(cx).store;
        let focused = self.search_focus.is_focused(window);
        let query = store.search().query.clone();
        let active = store.active_filters();
        let hint = if focused {
            "Buscar sessão".to_owned()
        } else {
            let first = keys::label(store, Action::Search).unwrap_or_default();
            format!("Buscar sessão  {first}")
        };
        let shown = if query.is_empty() { hint } else { query.clone() };
        let color = if query.is_empty() { mx.fg_faint } else { mx.fg };
        let caret = || div().flex_none().w(px(1.5)).h(px(14.)).bg(mx.accent);
        let mut line = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .overflow_hidden()
            .text_size(px(12.))
            .whitespace_nowrap();
        if focused && query.is_empty() {
            line = line.child(caret());
        }
        line = line.child(div().text_color(color).child(shown));
        if focused && !query.is_empty() {
            line = line.child(caret());
        }
        let clear_host = self.host.clone();
        let focus: FocusHandle = self.search_focus.clone();
        div()
            .id("sidebar-search")
            .track_focus(&self.search_focus)
            .key_context("SidebarSearch")
            .on_key_down(cx.listener(Self::on_search_key))
            .on_mouse_down(MouseButton::Left, move |_, window, _| window.focus(&focus))
            .flex_1()
            .h(px(28.))
            .pl(px(8.))
            .pr(px(2.))
            .rounded(px(8.))
            .border_1()
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_text()
            .bg(if focused { mx.bg_active } else { mx.bg_hover })
            .border_color(if focused { mx.accent } else { mx.border })
            .child(icon(Icon::Search, 14., if focused { mx.fg_dim } else { mx.fg_faint }))
            .child(line)
            .when(!query.is_empty(), |d| {
                d.child(mini_button(mx, "search-clear", Icon::CloseRounded, mx.fg_dim, "Limpar a busca", move |_, cx: &mut App| {
                    update_store(&clear_host, cx, |s| s.set_query(""))
                }))
            })
            .child(mini_button(
                mx,
                "search-filter",
                Icon::FilterList,
                if active > 0 { mx.accent } else { mx.fg_dim },
                filter_tooltip(active),
                |window, cx| menus::open(Target::Filter, window, cx),
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_tooltip_counts_active_filters() {
        assert_eq!(filter_tooltip(0), "Filtrar por pasta, feature/hotfix ou estado");
        assert_eq!(filter_tooltip(1), "1 filtro — clique pra mexer");
        assert_eq!(filter_tooltip(2), "2 filtros — clique pra mexer");
    }

    #[test]
    fn typing_appends_and_backspace_removes_the_last_char() {
        assert_eq!(edited("ab", "c", Some("c")).as_deref(), Some("abc"));
        assert_eq!(edited("ação", "backspace", None).as_deref(), Some("açã"));
        assert_eq!(edited("", "backspace", None), None);
        assert_eq!(edited("a", "tab", Some("\t")), None);
    }
}
