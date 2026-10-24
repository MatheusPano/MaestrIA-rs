use std::ops::Range;

use gpui::{
    App, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Pixels, Point, Styled, UTF16Selection, Window, canvas, div,
    prelude::FluentBuilder, point, px, size,
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

pub fn erased(query: &str) -> Option<String> {
    let mut q = query.to_owned();
    q.pop()?;
    Some(q)
}

fn byte_at(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (i, c) in text.char_indices() {
        if seen >= units {
            return i;
        }
        seen += c.len_utf16();
    }
    text.len()
}

fn units_at(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

pub fn spliced(query: &str, range: Option<Range<usize>>, marked: Option<Range<usize>>, text: &str) -> (String, Range<usize>) {
    let end = query.len();
    let r = range
        .map(|r| byte_at(query, r.start)..byte_at(query, r.end))
        .or(marked)
        .unwrap_or(end..end);
    let start = r.start.min(end);
    let stop = r.end.clamp(start, end);
    let text = text.replace(['\n', '\r', '\t'], " ");
    let mut q = query.to_owned();
    if !q.is_char_boundary(start) || !q.is_char_boundary(stop) {
        return (format!("{q}{text}"), end..end + text.len());
    }
    q.replace_range(start..stop, &text);
    (q, start..start + text.len())
}

impl Sidebar {
    pub(super) fn on_search_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        if k.modifiers.control || k.modifiers.platform || k.modifiers.alt {
            self.search_marked = None;
            return;
        }
        if k.key_char.is_none() {
            self.search_marked = None;
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
        if k.key != "backspace" {
            return;
        }
        if let Some(q) = erased(&store.search().query) {
            update_store(&self.host, cx, |s| s.set_query(&q));
            cx.stop_propagation();
        }
    }

    fn query(&self, cx: &App) -> String {
        self.host.read(cx).store.search().query.clone()
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
        let (input_focus, me) = (self.search_focus.clone(), cx.entity());
        let input = canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                me.update(cx, |s, _| s.search_bounds = bounds);
                window.handle_input(&input_focus, ElementInputHandler::new(bounds, me.clone()), cx);
            },
        )
        .absolute()
        .inset_0();
        div()
            .id("sidebar-search")
            .relative()
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
            .child(input)
    }
}

impl EntityInputHandler for Sidebar {
    fn text_for_range(&mut self, range: Range<usize>, adjusted: &mut Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) -> Option<String> {
        let q = self.query(cx);
        let r = byte_at(&q, range.start)..byte_at(&q, range.end);
        adjusted.replace(units_at(&q, r.start)..units_at(&q, r.end));
        Some(q[r].to_owned())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, cx: &mut Context<Self>) -> Option<UTF16Selection> {
        let end = units_at(&self.query(cx), usize::MAX);
        Some(UTF16Selection { range: end..end, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>> {
        let q = self.query(cx);
        self.search_marked.as_ref().map(|r| units_at(&q, r.start)..units_at(&q, r.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.search_marked = None;
    }

    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        if text.chars().any(char::is_control) {
            return;
        }
        let (q, _) = spliced(&self.query(cx), range, self.search_marked.take(), text);
        update_store(&self.host, cx, |s| s.set_query(&q));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (q, placed) = spliced(&self.query(cx), range, self.search_marked.take(), text);
        self.search_marked = (!placed.is_empty()).then_some(placed);
        update_store(&self.host, cx, |s| s.set_query(&q));
    }

    fn bounds_for_range(&mut self, _: Range<usize>, element: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let b = if self.search_bounds.size.width > px(0.) { self.search_bounds } else { element };
        Some(Bounds::new(point(b.origin.x + px(28.), b.origin.y + px(6.)), size(px(1.), px(16.))))
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
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
    fn backspace_removes_the_last_char() {
        assert_eq!(erased("ação").as_deref(), Some("açã"));
        assert_eq!(erased(""), None);
    }

    #[test]
    fn typing_appends_and_a_composed_accent_replaces_its_dead_key() {
        assert_eq!(spliced("ab", None, None, "c"), ("abc".to_owned(), 2..3));
        let (q, marked) = spliced("S", None, None, "~");
        assert_eq!((q.as_str(), marked.clone()), ("S~", 1..2));
        assert_eq!(spliced(&q, None, Some(marked), "ã").0, "Sã");
        assert_eq!(spliced("é😀", Some(1..3), None, "x").0, "éx");
    }
}
