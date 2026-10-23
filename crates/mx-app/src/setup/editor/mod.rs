mod buffer;
mod element;
mod input;
mod layout;

use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    ParentElement, Pixels, Point, Render, ScrollWheelEvent, Styled,
    Window, div, point, px,
};

pub use buffer::Buffer;
use element::EditorElement;
use layout::EditorLayout;

pub const PAD_LEFT: f32 = 18.;
pub const PAD_TOP: f32 = 14.;
pub const PAD_RIGHT: f32 = 18.;
pub const PAD_BOTTOM: f32 = 24.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    Changed,
    Save,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub fg: Hsla,
    pub caret: Hsla,
    pub selection: Hsla,
    pub thumb: Hsla,
}

impl Default for EditorStyle {
    fn default() -> Self {
        Self {
            font_size: 12.5,
            line_height: 1.55,
            fg: gpui::white(),
            caret: gpui::white(),
            selection: gpui::blue().opacity(0.3),
            thumb: gpui::white().opacity(0.2),
        }
    }
}

pub struct TextEditor {
    buffer: Buffer,
    focus: FocusHandle,
    style: EditorStyle,
    layout: Option<Rc<EditorLayout>>,
    bounds: Bounds<Pixels>,
    scroll_y: Pixels,
    autoscroll: bool,
    goal_x: Option<Pixels>,
    dragging: bool,
    drag_origin: usize,
}

impl EventEmitter<EditorEvent> for TextEditor {}

impl Focusable for TextEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextEditor {
    pub fn new(text: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self {
            buffer: Buffer::new(text),
            focus: cx.focus_handle(),
            style: EditorStyle::default(),
            layout: None,
            bounds: Bounds::default(),
            scroll_y: px(0.),
            autoscroll: false,
            goal_x: None,
            dragging: false,
            drag_origin: 0,
        }
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    #[cfg(test)]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.buffer.reset(text);
        self.scroll_y = px(0.);
        self.goal_x = None;
        cx.notify();
    }

    pub fn set_style(&mut self, style: EditorStyle, cx: &mut Context<Self>) {
        if self.style != style {
            self.style = style;
            cx.notify();
        }
    }

    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus);
    }

    fn line_height(&self) -> Pixels {
        px((self.style.font_size * self.style.line_height).round())
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.autoscroll = true;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    fn moved(&mut self, cx: &mut Context<Self>) {
        self.autoscroll = true;
        cx.notify();
    }

    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        self.buffer.insert(text);
        self.edited(cx);
    }

    fn to_content(&self, at: Point<Pixels>) -> Point<Pixels> {
        point(
            at.x - self.bounds.origin.x - px(PAD_LEFT),
            at.y - self.bounds.origin.y - px(PAD_TOP) + self.scroll_y,
        )
    }

    fn to_window(&self, at: Point<Pixels>) -> Point<Pixels> {
        point(
            at.x + self.bounds.origin.x + px(PAD_LEFT),
            at.y + self.bounds.origin.y + px(PAD_TOP) - self.scroll_y,
        )
    }

    fn offset_at_window(&self, at: Point<Pixels>) -> Option<usize> {
        Some(self.layout.as_ref()?.offset_at(self.to_content(at)))
    }

    fn vertical(&mut self, rows: f32, extend: bool, cx: &mut Context<Self>) {
        let head = self.buffer.head();
        let target = match &self.layout {
            Some(layout) => {
                let p = layout.position(head);
                let x = *self.goal_x.get_or_insert(p.x);
                let y = p.y + layout.line_height * (rows + 0.5);
                if y < px(0.) {
                    0
                } else {
                    layout.offset_at(point(x, y))
                }
            }
            None => self.buffer.vertical(head, rows > 0.),
        };
        let goal = self.goal_x;
        self.buffer.set_cursor(target, extend);
        self.goal_x = goal;
        self.moved(cx);
    }

    fn page_rows(&self) -> f32 {
        (f32::from(self.bounds.size.height) / f32::from(self.line_height())).floor().max(1.)
    }

    fn horizontal(&mut self, right: bool, extend: bool, word: bool, cx: &mut Context<Self>) {
        let b = &self.buffer;
        let target = if b.has_selection() && !extend && !word {
            if right { b.selection().end } else { b.selection().start }
        } else {
            match (right, word) {
                (true, true) => b.word_after(b.head()),
                (true, false) => b.next(b.head()),
                (false, true) => b.word_before(b.head()),
                (false, false) => b.prev(b.head()),
            }
        };
        self.buffer.set_cursor(target, extend);
        self.goal_x = None;
        self.moved(cx);
    }

    fn jump(&mut self, to: usize, extend: bool, cx: &mut Context<Self>) {
        self.buffer.set_cursor(to, extend);
        self.goal_x = None;
        self.moved(cx);
    }

    fn copy(&self, cx: &mut Context<Self>) {
        if self.buffer.has_selection() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.buffer.selected_text().to_owned()));
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
            self.insert(&text.replace("\r\n", "\n"), cx);
        }
    }

    fn key_down(&mut self, ev: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        let cmd = m.control || m.platform;
        let shift = m.shift;
        let word = m.control || m.alt;
        let len = self.buffer.text().len();
        let head = self.buffer.head();
        match k.key.as_str() {
            "left" => self.horizontal(false, shift, word, cx),
            "right" => self.horizontal(true, shift, word, cx),
            "up" => self.vertical(-1., shift, cx),
            "down" => self.vertical(1., shift, cx),
            "pageup" => self.vertical(-self.page_rows(), shift, cx),
            "pagedown" => self.vertical(self.page_rows(), shift, cx),
            "home" if cmd => self.jump(0, shift, cx),
            "end" if cmd => self.jump(len, shift, cx),
            "home" => self.jump(self.buffer.smart_home(head), shift, cx),
            "end" => self.jump(self.buffer.line_end(head), shift, cx),
            "backspace" => {
                self.buffer.backspace(word);
                self.edited(cx);
            }
            "delete" => {
                self.buffer.delete(word);
                self.edited(cx);
            }
            "enter" if !cmd => self.insert("\n", cx),
            "tab" if !cmd && !shift => self.insert("  ", cx),
            "a" if cmd => {
                self.buffer.select_all();
                cx.notify();
            }
            "c" if cmd => self.copy(cx),
            "x" if cmd => {
                if let Some(text) = self.buffer.cut() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.edited(cx);
                }
            }
            "v" if cmd => self.paste(cx),
            "z" if cmd && shift => {
                if self.buffer.redo() {
                    self.edited(cx);
                }
            }
            "z" if cmd => {
                if self.buffer.undo() {
                    self.edited(cx);
                }
            }
            "y" if cmd => {
                if self.buffer.redo() {
                    self.edited(cx);
                }
            }
            "s" if cmd => cx.emit(EditorEvent::Save),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn mouse_down(&mut self, at: Point<Pixels>, clicks: usize, shift: bool, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        let Some(offset) = self.offset_at_window(at) else {
            return;
        };
        match clicks {
            2 => self.buffer.select(self.buffer.word_at(offset)),
            n if n >= 3 => self.buffer.select(self.buffer.line_at(offset)),
            _ => self.buffer.set_cursor(offset, shift),
        }
        self.drag_origin = if shift { self.buffer.anchor() } else { self.buffer.selection().start };
        self.dragging = clicks == 1;
        self.goal_x = None;
        cx.notify();
    }

    fn mouse_drag(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.dragging {
            return;
        }
        if let Some(offset) = self.offset_at_window(at) {
            self.buffer.select(self.drag_origin..offset);
            self.moved(cx);
        }
    }

    fn mouse_up(&mut self) {
        self.dragging = false;
    }

    fn scroll_by(&mut self, dy: Pixels, cx: &mut Context<Self>) {
        let max = self.max_scroll();
        let next = (self.scroll_y - dy).clamp(px(0.), max);
        if next != self.scroll_y {
            self.scroll_y = next;
            cx.notify();
        }
    }

    fn max_scroll(&self) -> Pixels {
        let content = self.layout.as_ref().map_or(px(0.), |l| l.height) + px(PAD_TOP + PAD_BOTTOM);
        (content - self.bounds.size.height).max(px(0.))
    }

    fn adopt_layout(&mut self, layout: Rc<EditorLayout>, bounds: Bounds<Pixels>) {
        self.layout = Some(layout.clone());
        self.bounds = bounds;
        if self.autoscroll {
            self.autoscroll = false;
            let caret = layout.position(self.buffer.head()).y + px(PAD_TOP);
            let lh = layout.line_height;
            let view = bounds.size.height;
            if caret < self.scroll_y + lh {
                self.scroll_y = caret - lh;
            } else if caret + lh * 2. > self.scroll_y + view {
                self.scroll_y = caret + lh * 2. - view;
            }
        }
        self.scroll_y = self.scroll_y.clamp(px(0.), self.max_scroll());
    }
}

impl Render for TextEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("setup-editor")
            .debug_selector(|| "setup-editor".into())
            .track_focus(&self.focus)
            .key_context("SetupEditor")
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    this.mouse_down(ev.position, ev.click_count, ev.modifiers.shift, window, cx)
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if ev.pressed_button == Some(MouseButton::Left) {
                    this.mouse_drag(ev.position, cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.mouse_up()))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, _| this.mouse_up()))
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, cx| {
                let dy = ev.delta.pixel_delta(this.line_height()).y;
                this.scroll_by(dy, cx);
            }))
            .cursor(CursorStyle::IBeam)
            .size_full()
            .child(EditorElement::new(cx.entity()))
    }
}

#[cfg(test)]
mod tests;
