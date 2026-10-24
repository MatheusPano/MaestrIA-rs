use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use mx_rfw::{Map, Value};
use mx_theme::Theme;

use super::ActionSink;
use super::code_editor::CodeSpec;
use super::icons::named;
use super::layout::fire;
use super::style::{alpha, hsla};
use super::text_input::{InputColors, TextInput};
use crate::setup::editor::{EditorEvent, EditorStyle, TextEditor};

pub const CHANGE_DELAY: Duration = Duration::from_millis(350);
const MAX_FOUND: usize = 10_000;

struct FindBar {
    query: Entity<TextInput>,
    replace: Option<Entity<TextInput>>,
    found: Vec<Range<usize>>,
    current: Option<usize>,
    _watch: Vec<Subscription>,
}

pub struct CodePane {
    doc: String,
    editor: Entity<TextEditor>,
    spec: CodeSpec,
    sink: ActionSink,
    theme: Arc<Theme>,
    pending: u64,
    waiting: bool,
    last_cursor: Option<(usize, usize, usize)>,
    find: Option<FindBar>,
    _events: Subscription,
}

pub fn find_all(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let fold = query.is_ascii() && !query.chars().any(|c| c.is_ascii_uppercase());
    let (hay, needle) = if fold { (text.to_ascii_lowercase(), query.to_ascii_lowercase()) } else { (text.to_owned(), query.to_owned()) };
    hay.match_indices(&needle).take(MAX_FOUND).map(|(i, m)| i..i + m.len()).collect()
}

fn values(pairs: impl IntoIterator<Item = (&'static str, Value)>) -> Map {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

impl CodePane {
    pub fn new(spec: CodeSpec, style: EditorStyle, sink: ActionSink, theme: Arc<Theme>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = spec.text.clone().unwrap_or_default();
        let editor = cx.new(|cx| {
            let mut e = TextEditor::new(text, cx);
            e.set_style(style, cx);
            e.set_read_only(spec.read_only);
            e
        });
        let events = cx.subscribe_in(&editor, window, |this, _, ev: &EditorEvent, window, cx| match ev {
            EditorEvent::Changed => this.changed(window, cx),
            EditorEvent::Save => this.save(window, cx),
            EditorEvent::Cursor => this.cursor(window, cx),
        });
        let jump = spec.jump;
        let mut pane = Self {
            doc: spec.doc.clone(),
            editor,
            spec,
            sink,
            theme,
            pending: 0,
            waiting: false,
            last_cursor: None,
            find: None,
            _events: events,
        };
        pane.spec.jump = 0;
        pane.jump_to(jump, window, cx);
        pane.editor.read(cx).focus(window);
        pane
    }

    pub fn editor(&self) -> &Entity<TextEditor> {
        &self.editor
    }

    pub fn apply(&mut self, spec: CodeSpec, style: EditorStyle, sink: ActionSink, reappeared: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.sink = sink;
        let text = spec.text.clone();
        let reload = spec.revision != self.spec.revision
            || (reappeared && !self.waiting && text.as_deref().is_some_and(|t| t != self.editor.read(cx).text()));
        let jump = spec.jump;
        let old_jump = self.spec.jump;
        self.spec = CodeSpec { jump: old_jump, ..spec };
        let read_only = self.spec.read_only;
        self.editor.update(cx, |e, cx| {
            e.set_style(style, cx);
            e.set_read_only(read_only);
            if reload {
                e.reload(text.unwrap_or_default(), cx);
            }
        });
        if reload {
            self.refind(cx);
        }
        self.jump_to(jump, window, cx);
        if reappeared {
            self.last_cursor = None;
            self.editor.read(cx).focus(window);
        }
    }

    fn jump_to(&mut self, jump: i64, window: &mut Window, cx: &mut Context<Self>) {
        if jump == self.spec.jump {
            return;
        }
        self.spec.jump = jump;
        let Some(line) = self.spec.line else { return };
        let column = self.spec.column.unwrap_or(1);
        self.editor.update(cx, |e, cx| e.go_to(line.max(1) as usize, column.max(1) as usize, cx));
        self.editor.read(cx).focus(window);
    }

    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending += 1;
        self.waiting = true;
        let generation = self.pending;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CHANGE_DELAY).await;
            this.update_in(cx, |this, window, cx| {
                if this.pending == generation {
                    this.flush(window, cx);
                }
            })
            .ok();
        })
        .detach();
        if self.find.is_some() {
            self.refind(cx);
        }
    }

    fn text_values(&self, cx: &Context<Self>) -> Map {
        let text = self.editor.read(cx).text().to_owned();
        values([("doc", Value::from(self.doc.as_str())), ("text", Value::from(text))])
    }

    fn flush(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.waiting = false;
        let extra = self.text_values(cx);
        fire(&self.sink, &self.spec.on_change, &extra, window, cx);
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.waiting {
            self.pending += 1;
            self.flush(window, cx);
        }
        let extra = self.text_values(cx);
        fire(&self.sink, &self.spec.on_save, &extra, window, cx);
    }

    fn cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = self.editor.read(cx).cursor_position();
        if self.last_cursor == Some(now) {
            return;
        }
        self.last_cursor = Some(now);
        let (line, column, selected) = now;
        let extra = values([
            ("doc", Value::from(self.doc.as_str())),
            ("line", Value::Int(line as i64)),
            ("column", Value::Int(column as i64)),
            ("selected", Value::Int(selected as i64)),
        ]);
        fire(&self.sink, &self.spec.on_cursor, &extra, window, cx);
    }

    fn input(&self, text: String, placeholder: &str, cx: &mut Context<Self>) -> Entity<TextInput> {
        let colors = InputColors { fg: hsla(self.theme.fg), faint: hsla(self.theme.fg_faint), caret: hsla(self.theme.accent) };
        let placeholder = placeholder.to_owned();
        cx.new(|cx| {
            let mut t = TextInput::new(text, colors, cx);
            t.placeholder = Some(placeholder);
            t.size = 12.5;
            t
        })
    }

    pub fn open_find(&mut self, with_replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        let picked = self.editor.read(cx).selected_text().to_owned();
        let seed = (!picked.is_empty() && !picked.contains('\n')).then_some(picked);
        if self.find.is_none() {
            let query = self.input(seed.clone().unwrap_or_default(), "buscar", cx);
            let watch = vec![cx.observe(&query, |this, _, cx| this.refind(cx))];
            self.find = Some(FindBar { query, replace: None, found: Vec::new(), current: None, _watch: watch });
        } else if let (Some(s), Some(bar)) = (seed, &self.find) {
            bar.query.update(cx, |q, cx| {
                q.text = s;
                cx.notify();
            });
        }
        if with_replace && self.find.as_ref().is_some_and(|b| b.replace.is_none()) && !self.spec.read_only {
            let replace = self.input(String::new(), "trocar por", cx);
            if let Some(bar) = &mut self.find {
                bar.replace = Some(replace);
            }
        }
        if let Some(bar) = &self.find {
            window.focus(&bar.query.read(cx).focus_handle(cx));
        }
        self.refind(cx);
        cx.notify();
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        self.editor.update(cx, |e, cx| e.set_found(Vec::new(), None, cx));
        self.editor.read(cx).focus(window);
        cx.notify();
    }

    fn refind(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &mut self.find else { return };
        let query = bar.query.read(cx).text.clone();
        let editor = self.editor.read(cx);
        let found = find_all(editor.text(), &query);
        let from = editor.selection().start;
        let current = (!found.is_empty()).then(|| found.iter().position(|r| r.start >= from).unwrap_or(0));
        bar.found = found.clone();
        bar.current = current;
        self.editor.update(cx, |e, cx| e.set_found(found, None, cx));
        cx.notify();
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(bar) = &mut self.find else { return };
        let n = bar.found.len();
        if n == 0 {
            return;
        }
        let editor = self.editor.read(cx);
        let sel = editor.selection();
        let next = match bar.current {
            Some(i) if bar.found.get(i) == Some(&sel) => if forward { (i + 1) % n } else { (i + n - 1) % n },
            Some(i) => i,
            None => 0,
        };
        bar.current = Some(next);
        let found = bar.found.clone();
        self.editor.update(cx, |e, cx| e.set_found(found, Some(next), cx));
        cx.notify();
    }

    fn replace_one(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else { return };
        let with = bar.replace.as_ref().map(|r| r.read(cx).text.clone()).unwrap_or_default();
        let sel = self.editor.read(cx).selection();
        if bar.current.and_then(|i| bar.found.get(i)) == Some(&sel) {
            self.editor.update(cx, |e, cx| e.replace_range(sel, &with, cx));
            self.refind(cx);
        }
        self.step(true, cx);
    }

    fn replace_every(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else { return };
        let with = bar.replace.as_ref().map(|r| r.read(cx).text.clone()).unwrap_or_default();
        let found = bar.found.clone();
        self.editor.update(cx, |e, cx| e.replace_all(&found, &with, cx));
        self.refind(cx);
    }

    #[cfg(test)]
    pub fn replace_every_with(&mut self, with: &str, cx: &mut Context<Self>) {
        if let Some(r) = self.find.as_ref().and_then(|b| b.replace.clone()) {
            r.update(cx, |r, _| r.text = with.to_owned());
        }
        self.replace_every(cx);
    }

    fn key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        let cmd = (m.control || m.platform) && !m.alt;
        match k.key.as_str() {
            "f" if cmd && !m.shift => self.open_find(false, window, cx),
            "h" if cmd && !m.shift => self.open_find(true, window, cx),
            "g" if cmd && self.find.is_some() => self.step(!m.shift, cx),
            "f3" if self.find.is_some() => self.step(!m.shift, cx),
            "escape" if self.find.is_some() => self.close_find(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn bar_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        if k.key != "enter" {
            return;
        }
        let in_replace = self
            .find
            .as_ref()
            .and_then(|b| b.replace.as_ref())
            .is_some_and(|r| r.read(cx).focused(window));
        if in_replace {
            self.replace_one(cx);
        } else {
            self.step(!k.modifiers.shift, cx);
        }
        cx.stop_propagation();
    }
}

fn button(id: &'static str, icon: &str, color: gpui::Hsla, hover: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .size(px(22.0))
        .rounded(px(4.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(named(icon, 14.0, color))
}

fn text_button(id: &'static str, label: &'static str, p: &Theme) -> gpui::Stateful<gpui::Div> {
    let hover = hsla(p.bg_hover);
    div()
        .id(id)
        .flex_none()
        .px(px(8.0))
        .py(px(3.0))
        .rounded(px(4.0))
        .text_size(px(11.5))
        .text_color(hsla(p.fg_dim))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label)
}

fn field(input: &Entity<TextInput>, p: &Theme) -> gpui::Div {
    div()
        .flex_1()
        .min_w(px(0.0))
        .h(px(24.0))
        .px(px(8.0))
        .flex()
        .items_center()
        .rounded(px(5.0))
        .border_1()
        .border_color(hsla(p.border))
        .bg(hsla(p.bg))
        .child(input.clone())
}

impl CodePane {
    fn render_bar(&self, cx: &mut Context<Self>) -> Option<gpui::Stateful<gpui::Div>> {
        let bar = self.find.as_ref()?;
        let p = self.theme.clone();
        let (dim, hover) = (hsla(p.fg_dim), hsla(p.bg_hover));
        let count = match (bar.current, bar.found.len()) {
            (_, 0) if bar.query.read(cx).text.is_empty() => String::new(),
            (_, 0) => "sem resultados".to_owned(),
            (Some(i), n) => format!("{} de {n}", i + 1),
            (None, n) => n.to_string(),
        };
        let first = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(field(&bar.query, &p))
            .child(div().flex_none().min_w(px(70.0)).text_size(px(11.5)).text_color(dim).child(count))
            .child(button("find-prev", "north_rounded", dim, hover).on_click(cx.listener(|this, _, _, cx| this.step(false, cx))))
            .child(button("find-next", "south_rounded", dim, hover).on_click(cx.listener(|this, _, _, cx| this.step(true, cx))))
            .child(button("find-close", "close_rounded", dim, hover).on_click(cx.listener(|this, _, window, cx| this.close_find(window, cx))));
        let second = bar.replace.as_ref().map(|r| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(field(r, &p))
                .child(text_button("replace-one", "trocar", &p).on_click(cx.listener(|this, _, _, cx| this.replace_one(cx))))
                .child(text_button("replace-all", "trocar todos", &p).on_click(cx.listener(|this, _, _, cx| this.replace_every(cx))))
        });
        Some(
            div()
                .id("find-bar")
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .px(px(10.0))
                .py(px(6.0))
                .bg(hsla(p.bg_sidebar))
                .border_b_1()
                .border_color(alpha(p.border, 1.0))
                .capture_key_down(cx.listener(Self::bar_key))
                .child(first)
                .children(second),
        )
    }
}

impl Render for CodePane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bar = self.render_bar(cx);
        div()
            .id("code-pane")
            .key_context("CodeEditor")
            .on_key_down(cx.listener(Self::key))
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .children(bar)
            .child(div().flex_1().min_h(px(0.0)).child(self.editor.clone()))
    }
}
