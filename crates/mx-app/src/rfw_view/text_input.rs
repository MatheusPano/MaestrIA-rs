use std::rc::Rc;

use gpui::{
    App, ClipboardItem, Context, FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Render, Styled, Window, div, px, relative,
};

pub type Submit = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct TextInput {
    pub text: String,
    pub placeholder: Option<String>,
    pub multiline: bool,
    pub disabled: bool,
    pub size: f32,
    pub colors: InputColors,
    pub submit: Option<Submit>,
    focus: FocusHandle,
}

#[derive(Clone, Copy)]
pub struct InputColors {
    pub fg: Hsla,
    pub faint: Hsla,
    pub caret: Hsla,
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextInput {
    pub fn new(text: String, colors: InputColors, cx: &mut Context<Self>) -> Self {
        Self {
            text,
            placeholder: None,
            multiline: false,
            disabled: false,
            size: 13.0,
            colors,
            submit: None,
            focus: cx.focus_handle(),
        }
    }

    pub fn focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        let k = &e.keystroke;
        let ctrl = k.modifiers.control || k.modifiers.platform;
        match k.key.as_str() {
            "backspace" => {
                self.text.pop();
            }
            "enter" if self.multiline && !ctrl => self.text.push('\n'),
            "enter" => {
                if let Some(submit) = self.submit.clone() {
                    submit(window, cx);
                }
                return;
            }
            "v" if ctrl => {
                if let Some(t) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    let t = if self.multiline { t } else { t.replace('\n', " ") };
                    self.text.push_str(&t);
                }
            }
            "c" if ctrl => cx.write_to_clipboard(ClipboardItem::new_string(self.text.clone())),
            "u" if ctrl => self.text.clear(),
            _ => match &k.key_char {
                Some(ch) if !ctrl && !ch.chars().any(char::is_control) => self.text.push_str(ch),
                _ => return,
            },
        }
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        let lines = if self.multiline { 3.0 } else { 1.0 };
        let empty = self.text.is_empty();
        let shown = if empty { self.placeholder.clone().unwrap_or_default() } else { self.text.clone() };
        let color = if empty { self.colors.faint } else { self.colors.fg };
        let caret_color = self.colors.caret;
        let size = self.size;
        let caret = move || div().w(px(1.5)).h(px(size * 1.25)).bg(caret_color).flex_none();
        let focus = self.focus.clone();
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(MouseButton::Left, move |_, window, _| window.focus(&focus))
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(self.size * 1.4 * lines))
            .max_h(px(self.size * 1.4 * if self.multiline { 12.0 } else { 1.0 }))
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_start()
            .text_size(px(self.size))
            .line_height(relative(1.4))
            .text_color(color)
            .children((empty && focused).then(caret))
            .child(div().when_text(self.multiline).child(shown))
            .children((!empty && focused).then(caret))
    }
}

trait WhenText {
    fn when_text(self, multiline: bool) -> Self;
}

impl WhenText for gpui::Div {
    fn when_text(self, multiline: bool) -> Self {
        if multiline { self.flex_1().whitespace_normal() } else { self.whitespace_nowrap() }
    }
}
