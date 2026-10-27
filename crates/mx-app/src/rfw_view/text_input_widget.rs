mod commands;
mod field;
mod input;
mod keys;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AppContext, Bounds, Context, CursorStyle, Entity, FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Point, Render, StatefulInteractiveElement,
    Styled, Window, div, point, px,
};
use mx_rfw::widgets::text_input::{ClearSeen, SEND_TOOLTIP, TextInput};
use mx_rfw::{LocalLibrary, Map, Node, Widget};
use mx_theme::Theme;

use super::ActionSink;
use super::icons::{named, plugin_icon};
use super::layout::{Ctx, El, fire, owned};
use super::style::{UI_FONT, alpha, hsla};
use super::tooltip;
use super::view::RfwView;
use crate::setup::editor::Buffer;

pub use field::FieldLayout;
use field::{FieldElement, Shown, line_height};

#[derive(Clone, Copy)]
pub struct FieldColors {
    pub fg: Hsla,
    pub faint: Hsla,
    pub caret: Hsla,
    pub selection: Hsla,
    pub accent: Hsla,
    pub border: Hsla,
    pub fill: Hsla,
    pub list: Hsla,
    pub hover: Hsla,
}

impl FieldColors {
    pub fn of(t: &Theme) -> Self {
        Self {
            fg: hsla(t.fg),
            faint: hsla(t.fg_faint),
            caret: hsla(t.accent),
            selection: alpha(t.accent, 0.3),
            accent: hsla(t.accent),
            border: hsla(t.border),
            fill: hsla(t.bg_active),
            list: hsla(t.bg_sidebar),
            hover: hsla(t.bg_hover),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub font_size: f32,
    pub lines: i64,
    pub multiline: bool,
    pub disabled: bool,
}

pub struct FieldView {
    node: Node,
    pub(super) buffer: Buffer,
    pub(super) focus: FocusHandle,
    pub(super) colors: FieldColors,
    pub(super) spec: Look,
    pub(super) layout: Option<Rc<FieldLayout>>,
    pub(super) scroll_y: Pixels,
    bounds: Bounds<Pixels>,
    autoscroll: bool,
    pick: usize,
    dismissed: Option<String>,
    sink: ActionSink,
    theme: Arc<Theme>,
    dragging: Option<usize>,
}

impl Focusable for FieldView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

fn look(t: &TextInput) -> Look {
    Look { font_size: t.font_size as f32, lines: t.lines(), multiline: t.multiline, disabled: t.disabled }
}

impl FieldView {
    pub fn new(node: Node, theme: Arc<Theme>, sink: ActionSink, cx: &mut Context<Self>) -> Self {
        let t = TextInput::from_node(&node);
        let text = t.value.clone().unwrap_or_default();
        let spec = look(&t);
        let mut buffer = Buffer::new(text);
        let end = buffer.text().len();
        buffer.set_cursor(end, false);
        Self {
            node,
            buffer,
            focus: cx.focus_handle(),
            colors: FieldColors::of(&theme),
            spec,
            layout: None,
            scroll_y: px(0.),
            bounds: Bounds::default(),
            autoscroll: false,
            pick: 0,
            dismissed: None,
            sink,
            theme,
            dragging: None,
        }
    }

    pub fn input(&self) -> TextInput<'_> {
        TextInput::from_node(&self.node)
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    pub fn apply(&mut self, node: Node, theme: &Arc<Theme>, sink: ActionSink, clear: bool, cx: &mut Context<Self>) {
        if clear {
            self.buffer.reset("");
            self.edited(cx);
        }
        let changed = node != self.node || !Arc::ptr_eq(theme, &self.theme);
        self.node = node;
        self.sink = sink;
        if changed {
            self.spec = look(&self.input());
            self.colors = FieldColors::of(theme);
            self.theme = theme.clone();
            self.layout = None;
            cx.notify();
        }
    }

    pub(super) fn shown(&self) -> Shown {
        let lh = line_height(self.spec.font_size);
        if self.buffer.text().is_empty() {
            let t = self.input();
            Shown {
                text: t.shown_hint().unwrap_or_default().to_owned(),
                font_size: px(self.spec.font_size - 0.5),
                color: self.colors.faint,
                line_height: lh,
            }
        } else {
            Shown { text: self.buffer.text().to_owned(), font_size: px(self.spec.font_size), color: self.colors.fg, line_height: lh }
        }
    }

    pub(super) fn adopt(&mut self, layout: Rc<FieldLayout>, bounds: Bounds<Pixels>) {
        self.bounds = bounds;
        if std::mem::take(&mut self.autoscroll) && !self.buffer.text().is_empty() {
            let y = layout.position(self.buffer.head()).y;
            let lh = layout.line_height;
            if y < self.scroll_y {
                self.scroll_y = y;
            } else if y + lh > self.scroll_y + bounds.size.height {
                self.scroll_y = y + lh - bounds.size.height;
            }
        }
        let max = (layout.height - bounds.size.height).max(px(0.));
        self.scroll_y = self.scroll_y.clamp(px(0.), max);
        self.layout = Some(layout);
    }

    pub(super) fn edited(&mut self, cx: &mut Context<Self>) {
        self.pick = 0;
        self.autoscroll = true;
        cx.notify();
    }

    #[cfg(test)]
    pub fn type_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.buffer.insert(text);
        self.edited(cx);
    }

    pub(super) fn moved(&mut self, cx: &mut Context<Self>) {
        self.autoscroll = true;
        cx.notify();
    }

    pub(super) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let t = self.input();
        let Some(extra) = t.submitted(self.buffer.text()) else { return };
        let events = owned(&t.on_submit);
        fire(&self.sink, &events, &extra, window, cx);
    }

    pub(super) fn fire_with(&self, events: Vec<mx_rfw::Event>, extra: &Map, window: &mut Window, cx: &mut Context<Self>) {
        fire(&self.sink, &events, extra, window, cx);
    }

    fn content_point(&self, at: Point<Pixels>) -> Point<Pixels> {
        point(at.x - self.bounds.origin.x, at.y - self.bounds.origin.y + self.scroll_y)
    }

    pub(super) fn offset_at_window(&self, at: Point<Pixels>) -> Option<usize> {
        Some(self.layout.as_ref()?.offset_at(self.content_point(at)))
    }

    pub(super) fn to_window(&self, at: Point<Pixels>) -> Point<Pixels> {
        point(at.x + self.bounds.origin.x, at.y + self.bounds.origin.y - self.scroll_y)
    }

    fn mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        if self.buffer.text().is_empty() {
            cx.notify();
            return;
        }
        let Some(at) = self.offset_at_window(ev.position) else { return };
        match ev.click_count {
            2 => self.buffer.select(self.buffer.word_at(at)),
            n if n >= 3 => self.buffer.select_all(),
            _ => self.buffer.set_cursor(at, ev.modifiers.shift),
        }
        self.dragging = (ev.click_count == 1).then(|| self.buffer.anchor());
        cx.notify();
    }

    fn mouse_move(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(origin) = self.dragging.filter(|_| ev.pressed_button == Some(MouseButton::Left)) else { return };
        if let Some(at) = self.offset_at_window(ev.position) {
            self.buffer.select(origin..at);
            self.moved(cx);
        }
    }

    fn send_button(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let t = self.input();
        let icon = plugin_icon(t.send_icon.as_deref())?;
        let color = if t.disabled { self.colors.faint } else { self.colors.accent };
        let mut b = div()
            .id("text-input-send")
            .flex_none()
            .size(px(30.))
            .ml(px(4.))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .child(named(icon, 17., color))
            .tooltip(tooltip::builder(SEND_TOOLTIP.to_owned(), &self.theme));
        if !t.disabled {
            let hover = alpha(self.theme.fg, 0.08);
            b = b.cursor_pointer().hover(move |s| s.bg(hover)).on_click(cx.listener(|v, _, window, cx| {
                cx.stop_propagation();
                v.submit(window, cx);
            }));
        }
        Some(b.into_any_element())
    }
}

impl Render for FieldView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        let c = self.colors;
        let send = self.send_button(cx);
        let list = commands::list(self, cx);
        let mut field = div()
            .id("text-input-field")
            .track_focus(&self.focus)
            .key_context("PluginTextInput")
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(|v, ev: &MouseDownEvent, window, cx| v.mouse_down(ev, window, cx)))
            .on_mouse_move(cx.listener(|v, ev: &MouseMoveEvent, _, cx| v.mouse_move(ev, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(|v, _, _, _| v.dragging = None))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|v, _, _, _| v.dragging = None))
            .flex()
            .flex_row()
            .items_center()
            .bg(c.fill)
            .rounded(px(10.))
            .pl(px(12.))
            .pt(px(10.))
            .pr(px(8.))
            .pb(px(10.))
            .font_family(UI_FONT)
            .child(div().flex_1().min_w(px(0.)).cursor(CursorStyle::IBeam).child(FieldElement::new(cx.entity())))
            .children(send);
        field = if focused { field.border(px(1.4)).border_color(c.accent) } else { field.border_1().border_color(c.border) };
        div().flex().flex_col().w_full().children(list).child(field)
    }
}

#[derive(Default)]
pub struct TextInputs {
    fields: RefCell<HashMap<String, Entity<FieldView>>>,
    cleared: RefCell<ClearSeen>,
}

fn nodes(tree: &Widget) -> Vec<Node> {
    let mut out = Vec::new();
    tree.walk(&mut |w| {
        if let Widget::Node(n) = w
            && n.library == LocalLibrary::Maestria
            && n.name == "TextInput"
        {
            out.push(n.clone());
        }
    });
    out
}

impl TextInputs {
    pub fn sync(&self, tree: &Widget, theme: &Arc<Theme>, sink: &ActionSink, window: &mut Window, cx: &mut Context<RfwView>) {
        for node in nodes(tree) {
            let t = TextInput::from_node(&node);
            let (id, clear, autofocus) = (t.id.clone(), t.clear, t.autofocus);
            let empty = self.cleared.borrow_mut().should_clear(&id, clear);
            let existing = self.fields.borrow().get(&id).cloned();
            match existing {
                Some(f) => f.update(cx, |f, cx| f.apply(node, theme, sink.clone(), empty, cx)),
                None => {
                    let (theme, sink) = (theme.clone(), sink.clone());
                    let f = cx.new(|cx| FieldView::new(node, theme, sink, cx));
                    if autofocus {
                        let handle = f.read(cx).focus.clone();
                        window.focus(&handle);
                    }
                    self.fields.borrow_mut().insert(id, f);
                }
            }
        }
    }

    pub fn field(&self, id: &str) -> Option<Entity<FieldView>> {
        self.fields.borrow().get(id).cloned()
    }
}

pub fn render(cx: &mut Ctx, t: &TextInput) -> El {
    let el = cx.el().flex().flex_col();
    let el = if cx.bounded_w { el.w_full() } else { el.min_w(px(160.)) };
    match cx.widgets.and_then(|w| w.inputs.field(&t.id)) {
        Some(f) => el.child(f),
        None => el,
    }
}
