use std::ops::Range;
use std::time::Duration;

use gpui::{
    App, AppContext, Bounds, ClipboardItem, Context, Div, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels, Point, Render,
    Stateful, StatefulInteractiveElement, Styled, Task, UTF16Selection, WeakEntity, Window, canvas, div, px,
};
use mx_term::{FindSession, Hit};
use mx_theme::Theme;

use crate::app::icons::{self, Icon as ShellIcon};
use crate::chrome::icon::{self, Icon};
use crate::chrome::style::{alpha, hsla, shadow, theme};
use crate::chrome::widgets::tip;
use crate::keymap::LINUX;
use crate::terminal_view::TerminalView;

pub const HINT: &str = "Buscar no painel";
const PREV_TIP: &str = "Anterior  ⏎";
pub const TYPE_DEBOUNCE: Duration = Duration::from_millis(100);
pub const OUTPUT_DEBOUNCE: Duration = Duration::from_millis(250);
pub const CURRENT_ALPHA: f64 = 0.62;
pub const OTHER_ALPHA: f64 = 0.24;

pub struct FindState {
    pub session: FindSession,
    refresh: Option<Task<()>>,
}

pub struct FindBar {
    focus: FocusHandle,
    text: String,
    all_selected: bool,
    marked: bool,
    debounce: Option<Task<()>>,
    term: WeakEntity<TerminalView>,
}

impl Focusable for FindBar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl FindBar {
    fn new(term: WeakEntity<TerminalView>, cx: &mut Context<Self>) -> Self {
        Self { focus: cx.focus_handle(), text: String::new(), all_selected: false, marked: false, debounce: None, term }
    }

    #[cfg(test)]
    pub fn text(&self) -> &str {
        &self.text
    }

    fn reopen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.all_selected = !self.text.is_empty();
        self.focus.focus(window);
        cx.notify();
    }

    fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.text = text;
        self.all_selected = false;
        let term = self.term.clone();
        self.debounce = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TYPE_DEBOUNCE).await;
            let _ = this.update(cx, |bar, cx| {
                bar.debounce = None;
                let query = bar.text.clone();
                let _ = term.update(cx, |v, cx| v.find_query(&query, cx));
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn step(&mut self, by: i32, cx: &mut Context<Self>) {
        let query = self.text.clone();
        let pending = self.debounce.take().is_some();
        let _ = self.term.update(cx, |v, cx| if pending { v.find_query(&query, cx) } else { v.find_step(by, cx) });
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.debounce = None;
        let _ = self.term.update(cx, |v, cx| v.close_find(window, cx));
    }

    fn insert(&mut self, s: &str, cx: &mut Context<Self>) {
        let mut text = if self.all_selected { String::new() } else { self.text.clone() };
        text.push_str(&s.replace(['\n', '\r'], " "));
        self.set_text(text, cx);
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        let command = m.control || m.platform;
        match k.key.as_str() {
            "enter" => self.step(if m.shift { 1 } else { -1 }, cx),
            "escape" => self.close(window, cx),
            "backspace" | "delete" if self.all_selected => self.set_text(String::new(), cx),
            "backspace" => {
                let mut text = self.text.clone();
                text.pop();
                self.set_text(text, cx);
            }
            "a" if command => {
                self.all_selected = !self.text.is_empty();
                cx.notify();
            }
            "c" if command && self.all_selected => cx.write_to_clipboard(ClipboardItem::new_string(self.text.clone())),
            "v" if command => {
                if let Some(s) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    self.insert(&s, cx);
                }
            }
            "left" | "right" | "home" | "end" => {
                self.all_selected = false;
                cx.notify();
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn field(&self, focused: bool, cx: &Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let caret = || div().flex_none().w(px(1.5)).h(px(14.)).bg(hsla(t.accent));
        let mut row = div().flex().items_center().h_full().overflow_hidden().whitespace_nowrap().text_size(px(12.));
        if self.text.is_empty() {
            if focused {
                row = row.child(caret());
            }
            return row.child(div().text_color(hsla(t.fg_faint)).child(HINT));
        }
        let body = div().text_color(hsla(t.fg)).child(self.text.clone());
        row = if self.all_selected { row.child(body.bg(alpha(t.accent, 0.35))) } else { row.child(body) };
        if focused && !self.all_selected {
            row = row.child(caret());
        }
        row
    }
}

fn button(t: &Theme, id: &'static str, glyph: Icon, hint: &'static str, enabled: bool) -> Stateful<Div> {
    let color = if enabled { hsla(t.fg_dim) } else { alpha(t.fg_faint, 0.4) };
    let (bg, fg) = (hsla(t.border), hsla(t.fg));
    let mut glyph = icon::icon(glyph, 16., color);
    if enabled {
        glyph = glyph.group_hover(id, move |s| s.text_color(fg));
    }
    let b = div()
        .id(id)
        .group(id)
        .flex_none()
        .size(px(24.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .child(glyph)
        .tooltip(tip(hint));
    if enabled { b.cursor_pointer().hover(move |s| s.bg(bg)) } else { b }
}

impl Render for FindBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let focused = self.focus.is_focused(window);
        let (counter, count) = self
            .term
            .upgrade()
            .and_then(|v| v.read(cx).find.as_ref().map(|f| (f.session.counter(), f.session.hits().len())))
            .unwrap_or((None, 0));
        let none = counter.is_some() && count == 0;
        let me = cx.entity();
        let focus = self.focus.clone();
        let input = canvas(|_, _, _| {}, move |bounds, _, window, cx| {
            window.handle_input(&focus, ElementInputHandler::new(bounds, me.clone()), cx);
        })
        .absolute()
        .inset_0();
        div()
            .id("find-bar")
            .debug_selector(|| "find-bar".into())
            .occlude()
            .track_focus(&self.focus)
            .key_context("TextField")
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(|bar, _, window, _| bar.focus.focus(window)))
            .w(px(300.))
            .h(px(34.))
            .pl(px(10.))
            .pr(px(4.))
            .flex()
            .items_center()
            .bg(hsla(t.bg_sidebar))
            .rounded(px(8.))
            .border_1()
            .border_color(hsla(if none { t.red } else { t.border }))
            .shadow(shadow(&t, 10., 3.))
            .child(icons::icon(ShellIcon::Search, 14., hsla(t.fg_faint)))
            .child(div().w(px(6.)).flex_none())
            .child(div().relative().flex_1().h_full().min_w_0().child(self.field(focused, cx)).child(input))
            .children(counter.map(|c| {
                div()
                    .px(px(6.))
                    .flex_none()
                    .text_size(px(11.))
                    .text_color(hsla(if none { t.red } else { t.fg_faint }))
                    .child(c)
            }))
            .child(
                button(&t, "find-prev", icon::KEYBOARD_ARROW_UP, PREV_TIP, count > 0)
                    .on_click(cx.listener(|bar, _, _, cx| bar.step(-1, cx))),
            )
            .child(
                button(&t, "find-next", icon::KEYBOARD_ARROW_DOWN, if LINUX { "Próximo  Shift+Enter" } else { "Próximo  ⇧⏎" }, count > 0)
                    .on_click(cx.listener(|bar, _, _, cx| bar.step(1, cx))),
            )
            .child(
                button(&t, "find-close", icon::CLOSE_ROUNDED, if LINUX { "Fechar  Esc" } else { "Fechar  ⎋" }, true)
                    .on_click(cx.listener(|bar, _, window, cx| bar.close(window, cx))),
            )
    }
}

impl EntityInputHandler for FindBar {
    fn text_for_range(&mut self, _: Range<usize>, _: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let end = self.text.encode_utf16().count();
        let start = if self.all_selected { 0 } else { end };
        Some(UTF16Selection { range: start..end, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = false;
    }

    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = false;
        if !text.chars().any(char::is_control) {
            self.insert(text, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        self.marked = true;
    }

    fn bounds_for_range(&mut self, _: Range<usize>, bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}

impl TerminalView {
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            self.find = Some(FindState { session: FindSession::default(), refresh: None });
        }
        let me = cx.entity().downgrade();
        let bar = self.find_bar.get_or_insert_with(|| cx.new(|cx| FindBar::new(me, cx))).clone();
        bar.update(cx, |b, cx| b.reopen(window, cx));
        cx.notify();
    }

    pub fn owns_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx)
            || self.find.is_some() && self.find_bar.as_ref().is_some_and(|b| b.read(cx).focus.contains_focused(window, cx))
    }

    pub(crate) fn find_query(&mut self, query: &str, cx: &mut Context<Self>) {
        let session = self.session.clone();
        let Some(find) = self.find.as_mut() else { return };
        find.session.search(&session, query, session.viewport().bottom());
        self.reveal_hit(cx);
    }

    pub(crate) fn find_step(&mut self, by: i32, cx: &mut Context<Self>) {
        let session = self.session.clone();
        let Some(find) = self.find.as_mut() else { return };
        find.session.step(&session, by);
        self.reveal_hit(cx);
    }

    pub(crate) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.take().is_some() {
            window.focus(&self.focus);
            self.repaint(cx);
        }
    }

    pub(crate) fn schedule_find_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(find) = self.find.as_mut() else { return };
        if find.refresh.is_some() || find.session.query().is_empty() {
            return;
        }
        find.refresh = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(OUTPUT_DEBOUNCE).await;
            let _ = this.update(cx, |v, cx| {
                let session = v.session.clone();
                if let Some(find) = v.find.as_mut() {
                    find.refresh = None;
                    find.session.refresh(&session);
                }
                if let Some(bar) = &v.find_bar {
                    bar.update(cx, |_, cx| cx.notify());
                }
                v.repaint(cx);
            });
        }));
    }

    pub(crate) fn find_marks(&self) -> Vec<(Hit, bool)> {
        let Some(find) = &self.find else { return Vec::new() };
        let current = find.session.index();
        find.session.hits().iter().enumerate().map(|(i, h)| (*h, Some(i) == current)).collect()
    }

    fn reveal_hit(&mut self, cx: &mut Context<Self>) {
        let current = self.find.as_ref().and_then(|f| f.session.current());
        if let Some(hit) = current {
            let view = self.session.viewport();
            if !view.shows(&hit) {
                self.session.scroll_to_top(view.centring(hit.from.y));
            }
        }
        self.repaint(cx);
    }

    fn repaint(&mut self, cx: &mut Context<Self>) {
        self.render.borrow_mut().painted = false;
        cx.notify();
    }
}
