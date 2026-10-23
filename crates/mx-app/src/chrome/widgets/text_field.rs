use gpui::{
    App, ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, SharedString, Styled, Window,
    div, px,
};

use crate::chrome::style::{MONO, hsla, theme};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextFieldEvent {
    Changed(String),
    Submit,
    Cancel,
}

pub struct TextField {
    focus: FocusHandle,
    text: String,
    cursor: usize,
    all_selected: bool,
    label: Option<SharedString>,
    hint: SharedString,
    lines: Option<(usize, usize)>,
    mono: bool,
    filled: bool,
    selector: SharedString,
}

impl EventEmitter<TextFieldEvent> for TextField {}

impl Focusable for TextField {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            text: String::new(),
            cursor: 0,
            all_selected: false,
            label: None,
            hint: SharedString::default(),
            lines: None,
            mono: false,
            filled: false,
            selector: SharedString::default(),
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        if self.selector.is_empty() {
            self.selector = label.clone();
        }
        self.label = Some(label);
        self
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = hint.into();
        self
    }

    pub fn lines(mut self, min: usize, max: usize) -> Self {
        self.lines = Some((min, max));
        self
    }

    pub fn filled(mut self) -> Self {
        self.filled = true;
        self
    }

    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn selector(mut self, selector: impl Into<SharedString>) -> Self {
        self.selector = selector.into();
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self.cursor = self.text.len();
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.text = text.into();
        self.cursor = self.text.len();
        self.all_selected = false;
        cx.notify();
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.all_selected = !self.text.is_empty();
        self.cursor = self.text.len();
        cx.notify();
    }

    fn take_selection(&mut self) -> bool {
        if !std::mem::take(&mut self.all_selected) {
            return false;
        }
        self.text.clear();
        self.cursor = 0;
        true
    }

    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus);
    }

    pub fn insert(&mut self, s: &str, cx: &mut Context<Self>) {
        let s = if self.lines.is_some() {
            s.to_owned()
        } else {
            s.replace(['\n', '\r'], " ")
        };
        self.take_selection();
        self.text.insert_str(self.cursor, &s);
        self.cursor += s.len();
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(TextFieldEvent::Changed(self.text.clone()));
        cx.notify();
    }

    fn prev(&self) -> usize {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .chars()
            .next()
            .map_or(self.cursor, |c| self.cursor + c.len_utf8())
    }

    fn key_down(&mut self, ev: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        let command = m.control || m.platform;
        if self.all_selected && !matches!(k.key.as_str(), "a" | "c" | "x" | "backspace" | "delete") && (command || k.key_char.is_none()) {
            self.all_selected = false;
            cx.notify();
        }
        match k.key.as_str() {
            "backspace" | "delete" if self.all_selected => {
                self.take_selection();
                self.changed(cx);
            }
            "a" if command => self.select_all(cx),
            "c" | "x" if command && !m.shift && self.all_selected => {
                cx.write_to_clipboard(ClipboardItem::new_string(self.text.clone()));
                if k.key == "x" {
                    self.take_selection();
                    self.changed(cx);
                }
            }
            "backspace" if self.cursor > 0 => {
                let at = self.prev();
                self.text.replace_range(at..self.cursor, "");
                self.cursor = at;
                self.changed(cx);
            }
            "delete" if self.cursor < self.text.len() => {
                let to = self.next();
                self.text.replace_range(self.cursor..to, "");
                self.changed(cx);
            }
            "left" => {
                self.cursor = self.prev();
                cx.notify();
            }
            "right" => {
                self.cursor = self.next();
                cx.notify();
            }
            "home" => {
                self.cursor = self.text[..self.cursor].rfind('\n').map_or(0, |i| i + 1);
                cx.notify();
            }
            "end" => {
                self.cursor = self.text[self.cursor..]
                    .find('\n')
                    .map_or(self.text.len(), |i| self.cursor + i);
                cx.notify();
            }
            "escape" => cx.emit(TextFieldEvent::Cancel),
            "enter" if self.lines.is_some() && !command => self.insert("\n", cx),
            "enter" => cx.emit(TextFieldEvent::Submit),
            "v" if command => {
                if let Some(s) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    self.insert(&s, cx);
                }
            }
            "c" if command && m.shift => {
                cx.write_to_clipboard(ClipboardItem::new_string(self.text.clone()));
            }
            _ => {
                if command || m.function {
                    return;
                }
                match &k.key_char {
                    Some(ch) if !ch.chars().any(char::is_control) => {
                        let ch = ch.clone();
                        self.insert(&ch, cx);
                    }
                    _ => return,
                }
            }
        }
        cx.stop_propagation();
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let focused = self.focus.is_focused(window);
        let size = 13.;
        let line_h = 18.;
        let (min, max) = self.lines.unwrap_or((1, 1));
        let caret = || div().w(px(1.5)).h(px(line_h - 2.)).bg(hsla(t.accent));
        let body = if self.text.is_empty() {
            let mut row = div().flex().items_center().h(px(line_h));
            if focused {
                row = row.child(caret());
            }
            div().child(row.child(
                div().text_size(px(12.)).text_color(hsla(t.fg_faint)).child(self.hint.clone()),
            ))
        } else {
            let mut col = div().flex().flex_col();
            let mut start = 0;
            for line in self.text.split('\n') {
                let end = start + line.len();
                let multiline = self.lines.is_some();
                let has_caret = focused && self.cursor >= start && self.cursor <= end;
                let row = if multiline && !has_caret && !self.all_selected {
                    div().min_h(px(line_h)).child(line.to_owned())
                } else {
                    let mut row = div().flex().items_center().min_h(px(line_h));
                    row = if multiline { row.flex_wrap() } else { row.whitespace_nowrap() };
                    if self.all_selected {
                        row.child(div().bg(hsla(t.accent).opacity(0.35)).child(line.to_owned()))
                    } else if has_caret {
                        let at = self.cursor - start;
                        row.child(line[..at].to_owned()).child(caret()).child(line[at..].to_owned())
                    } else {
                        row.child(line.to_owned())
                    }
                };
                col = col.child(row);
                start = end + 1;
            }
            col
        };
        let mut boxed = div()
            .relative()
            .px(px(12.))
            .py(px(10.))
            .min_h(px(min as f32 * line_h + 20.))
            .rounded(px(4.))
            .border_1()
            .border_color(hsla(if focused { t.accent } else { t.border }))
            .text_size(px(size))
            .line_height(px(line_h))
            .text_color(hsla(t.fg))
            .child(body);
        if self.lines.is_some() {
            boxed = boxed
                .max_h(px(max as f32 * line_h + 20.))
                .overflow_hidden();
        }
        if self.mono {
            boxed = boxed.font_family(MONO);
        }
        if self.filled {
            boxed = boxed.bg(hsla(t.bg));
        }
        let mut outer = div().relative().pt(px(6.)).child(boxed);
        if let Some(label) = &self.label {
            outer = outer.child(
                div()
                    .absolute()
                    .top(px(-1.))
                    .left(px(8.))
                    .px(px(4.))
                    .bg(hsla(t.bg_sidebar))
                    .text_size(px(12.))
                    .line_height(px(14.))
                    .text_color(hsla(if focused { t.accent } else { t.fg_dim }))
                    .child(label.clone()),
            );
        }
        let selector = self.selector.clone();
        div()
            .id(SharedString::from(format!("field:{selector}")))
            .debug_selector(move || format!("field:{selector}"))
            .track_focus(&self.focus)
            .key_context("TextField")
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, _| window.focus(&this.focus)),
            )
            .child(outer)
    }
}
