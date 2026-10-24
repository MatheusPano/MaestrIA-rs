mod buffer;
mod code;
mod element;
mod input;
mod keys;
mod layout;
pub mod syntax;

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point,
    Render, ScrollWheelEvent, Styled, Window, div, point, px,
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
    Cursor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxColors {
    pub comment: Hsla,
    pub string: Hsla,
    pub number: Hsla,
    pub keyword: Hsla,
    pub kind: Hsla,
    pub key: Hsla,
    pub meta: Hsla,
    pub added: Hsla,
    pub removed: Hsla,
}

impl SyntaxColors {
    pub fn of(&self, tone: syntax::Tone) -> Hsla {
        use syntax::Tone;
        match tone {
            Tone::Comment => self.comment,
            Tone::String => self.string,
            Tone::Number => self.number,
            Tone::Keyword => self.keyword,
            Tone::Type => self.kind,
            Tone::Key => self.key,
            Tone::Meta => self.meta,
            Tone::Added => self.added,
            Tone::Removed => self.removed,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub fg: Hsla,
    pub caret: Hsla,
    pub selection: Hsla,
    pub thumb: Hsla,
    pub wrap: bool,
    pub code: bool,
    pub language: String,
    pub syntax: Option<SyntaxColors>,
    pub gutter: Option<(Hsla, Hsla)>,
    pub found: Hsla,
    pub found_current: Hsla,
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
            wrap: true,
            code: false,
            language: String::new(),
            syntax: None,
            gutter: None,
            found: gpui::yellow().opacity(0.25),
            found_current: gpui::yellow().opacity(0.5),
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
    scroll_x: Pixels,
    autoscroll: bool,
    center_next: bool,
    goal_x: Option<Pixels>,
    dragging: bool,
    drag_origin: usize,
    read_only: bool,
    found: Vec<Range<usize>>,
    found_current: Option<usize>,
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
            scroll_x: px(0.),
            autoscroll: false,
            center_next: false,
            goal_x: None,
            dragging: false,
            drag_origin: 0,
            read_only: false,
            found: Vec::new(),
            found_current: None,
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
        self.scroll_x = px(0.);
        self.goal_x = None;
        self.found.clear();
        self.found_current = None;
        cx.notify();
    }

    pub fn set_style(&mut self, style: EditorStyle, cx: &mut Context<Self>) {
        if self.style != style {
            self.style = style;
            self.layout = None;
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
        cx.emit(EditorEvent::Cursor);
        cx.notify();
    }

    fn moved(&mut self, cx: &mut Context<Self>) {
        self.autoscroll = true;
        cx.emit(EditorEvent::Cursor);
        cx.notify();
    }

    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        self.buffer.insert(text);
        self.edited(cx);
    }

    fn left(&self) -> Pixels {
        px(PAD_LEFT) + self.layout.as_ref().map_or(px(0.), |l| l.gutter)
    }

    fn to_content(&self, at: Point<Pixels>) -> Point<Pixels> {
        point(
            at.x - self.bounds.origin.x - self.left() + self.scroll_x,
            at.y - self.bounds.origin.y - px(PAD_TOP) + self.scroll_y,
        )
    }

    fn to_window(&self, at: Point<Pixels>) -> Point<Pixels> {
        point(
            at.x + self.bounds.origin.x + self.left() - self.scroll_x,
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
        cx.emit(EditorEvent::Cursor);
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

    fn scroll_by(&mut self, delta: Point<Pixels>, cx: &mut Context<Self>) {
        let y = (self.scroll_y - delta.y).clamp(px(0.), self.max_scroll());
        let x = (self.scroll_x - delta.x).clamp(px(0.), self.max_scroll_x());
        if y != self.scroll_y || x != self.scroll_x {
            self.scroll_y = y;
            self.scroll_x = x;
            cx.notify();
        }
    }

    fn max_scroll(&self) -> Pixels {
        let content = self.layout.as_ref().map_or(px(0.), |l| l.height) + px(PAD_TOP + PAD_BOTTOM);
        (content - self.bounds.size.height).max(px(0.))
    }

    fn view_width(&self) -> Pixels {
        (self.bounds.size.width - self.left() - px(PAD_RIGHT)).max(px(0.))
    }

    fn max_scroll_x(&self) -> Pixels {
        if self.style.wrap {
            return px(0.);
        }
        let widest = self.layout.as_ref().map_or(px(0.), |l| l.width);
        (widest + px(PAD_RIGHT) - self.view_width()).max(px(0.))
    }

    fn adopt_layout(&mut self, layout: Rc<EditorLayout>, bounds: Bounds<Pixels>) {
        self.layout = Some(layout.clone());
        self.bounds = bounds;
        if self.autoscroll {
            self.autoscroll = false;
            let p = layout.position(self.buffer.head());
            let caret = p.y + px(PAD_TOP);
            let lh = layout.line_height;
            let view = bounds.size.height;
            if std::mem::take(&mut self.center_next) {
                self.scroll_y = caret - view / 2.;
            } else if caret < self.scroll_y + lh {
                self.scroll_y = caret - lh;
            } else if caret + lh * 2. > self.scroll_y + view {
                self.scroll_y = caret + lh * 2. - view;
            }
            let width = self.view_width();
            let margin = layout.font_size * 2.;
            if p.x < self.scroll_x + margin {
                self.scroll_x = p.x - margin;
            } else if p.x + margin > self.scroll_x + width {
                self.scroll_x = p.x + margin - width;
            }
        }
        self.scroll_y = self.scroll_y.clamp(px(0.), self.max_scroll());
        self.scroll_x = self.scroll_x.clamp(px(0.), self.max_scroll_x());
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
                let mut delta = ev.delta.pixel_delta(this.line_height());
                if ev.modifiers.shift && delta.x == px(0.) {
                    delta = point(delta.y, px(0.));
                }
                this.scroll_by(delta, cx);
            }))
            .cursor(CursorStyle::IBeam)
            .size_full()
            .child(EditorElement::new(cx.entity()))
    }
}

#[cfg(test)]
mod tests;
