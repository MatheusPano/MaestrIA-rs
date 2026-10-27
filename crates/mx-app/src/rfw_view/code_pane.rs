use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, Render, Styled, Subscription, Window, div, px,
};
use mx_rfw::{Map, Value};
use mx_theme::Theme;

use super::ActionSink;
use super::code_editor::{CodeMenu, CodeSpec, FindBar, menu_entries};
use super::layout::fire;
use super::style::hsla;
use crate::chrome::menu::ContextMenu;
use crate::setup::editor::{EditorEvent, EditorStyle, TextEditor};

pub const CHANGE_DELAY: Duration = Duration::from_millis(350);
pub const CURSOR_DELAY: Duration = Duration::from_millis(120);
const MAC: bool = cfg!(target_os = "macos");

type Spot = (Range<usize>, (usize, usize, usize));

pub struct CodePane {
    pub(super) doc: String,
    pub(super) editor: Entity<TextEditor>,
    pub(super) spec: CodeSpec,
    pub(super) theme: Arc<Theme>,
    pub(super) find: Option<FindBar>,
    sink: ActionSink,
    revision: i64,
    jump: Option<i64>,
    sent: u64,
    change_gen: u64,
    cursor_gen: u64,
    last_spot: Option<Spot>,
    menu: Option<Entity<ContextMenu<CodeMenu>>>,
    _events: Subscription,
}

fn values(pairs: impl IntoIterator<Item = (&'static str, Value)>) -> Map {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

impl CodePane {
    pub fn new(spec: &CodeSpec, theme: Arc<Theme>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = spec.text.clone().unwrap_or_default();
        let editor = cx.new(|cx| TextEditor::new(text, cx));
        let events = cx.subscribe_in(&editor, window, |this, _, ev: &EditorEvent, window, cx| match ev {
            EditorEvent::Changed => this.changed(window, cx),
            EditorEvent::Save => this.save(window, cx),
            EditorEvent::Cursor => this.cursor(window, cx),
        });
        let sent = editor.read(cx).version();
        Self {
            doc: spec.doc.clone(),
            editor,
            spec: spec.clone(),
            theme,
            find: None,
            sink: std::rc::Rc::new(|_, _, _| {}),
            revision: spec.revision,
            jump: None,
            sent,
            change_gen: 0,
            cursor_gen: 0,
            last_spot: None,
            menu: None,
            _events: events,
        }
    }

    pub fn editor(&self) -> &Entity<TextEditor> {
        &self.editor
    }

    fn configure(&mut self, spec: CodeSpec, style: EditorStyle, sink: ActionSink, cx: &mut Context<Self>) {
        self.sink = sink;
        self.doc.clone_from(&spec.doc);
        let read_only = spec.read_only;
        self.spec = spec;
        self.editor.update(cx, |e, cx| {
            e.set_style(style, cx);
            e.set_read_only(read_only);
        });
    }

    fn spot(&self, cx: &Context<Self>) -> Spot {
        let e = self.editor.read(cx);
        (e.selection(), e.cursor_position())
    }

    pub fn attach(&mut self, spec: CodeSpec, style: EditorStyle, sink: ActionSink, window: &mut Window, cx: &mut Context<Self>) {
        self.configure(spec, style, sink, cx);
        self.last_spot = Some(self.spot(cx));
        if self.spec.autofocus {
            self.editor.read(cx).focus(window);
        }
        self.sync(window, cx);
    }

    pub fn apply(&mut self, spec: CodeSpec, style: EditorStyle, sink: ActionSink, window: &mut Window, cx: &mut Context<Self>) {
        self.configure(spec, style, sink, cx);
        self.sync(window, cx);
    }

    pub fn detach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(window, cx);
        self.cursor_gen += 1;
        self.menu = None;
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let reload = self.spec.revision != self.revision && self.spec.text.is_some();
        let jump = self.spec.line.is_some() && self.jump != Some(self.spec.jump);
        if !reload && !jump {
            return;
        }
        self.revision = self.spec.revision;
        if jump {
            self.jump = Some(self.spec.jump);
        }
        let text = self.spec.text.clone().unwrap_or_default();
        if reload && self.editor.read(cx).text() != text {
            self.change_gen += 1;
            let line = self.editor.read(cx).cursor_line();
            self.editor.update(cx, |e, cx| {
                e.reload(text, cx);
                if !jump {
                    e.place(line, 0, false, cx);
                }
            });
            self.sent = self.editor.read(cx).version();
            self.refind(cx);
        }
        if jump {
            let line = self.spec.line.unwrap_or(1).max(1) as usize;
            let column = self.spec.column.unwrap_or(1).max(1) as usize;
            self.editor.update(cx, |e, cx| e.go_to(line, column, cx));
            self.editor.read(cx).focus(window);
        }
    }

    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.read(cx).version() != self.sent {
            self.change_gen += 1;
            let generation = self.change_gen;
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor().timer(CHANGE_DELAY).await;
                this.update_in(cx, |this, window, cx| {
                    if this.change_gen == generation {
                        this.flush(window, cx);
                    }
                })
                .ok();
            })
            .detach();
        }
        if self.find.is_some() {
            self.refind(cx);
        }
    }

    fn text_values(&self, cx: &Context<Self>) -> Map {
        let text = self.editor.read(cx).text().to_owned();
        values([("doc", Value::from(self.doc.as_str())), ("text", Value::from(text))])
    }

    fn flush(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_gen += 1;
        let version = self.editor.read(cx).version();
        if version == self.sent {
            return;
        }
        self.sent = version;
        let extra = self.text_values(cx);
        fire(&self.sink, &self.spec.on_change, &extra, window, cx);
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(window, cx);
        let extra = self.text_values(cx);
        fire(&self.sink, &self.spec.on_save, &extra, window, cx);
    }

    fn cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = self.spot(cx);
        if self.last_spot.as_ref() == Some(&now) {
            return;
        }
        self.last_spot = Some(now);
        self.cursor_gen += 1;
        let generation = self.cursor_gen;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CURSOR_DELAY).await;
            this.update_in(cx, |this, window, cx| {
                if this.cursor_gen == generation {
                    this.send_cursor(window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn send_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (line, column, selected) = self.editor.read(cx).cursor_position();
        let extra = values([
            ("doc", Value::from(self.doc.as_str())),
            ("line", Value::Int(line as i64)),
            ("column", Value::Int(column as i64)),
            ("selected", Value::Int(selected as i64)),
        ]);
        fire(&self.sink, &self.spec.on_cursor, &extra, window, cx);
    }

    pub fn open_menu(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let entries = menu_entries(self.editor.read(cx).has_selection(), MAC);
        let picked = cx.entity().downgrade();
        let dismissed = picked.clone();
        self.menu = Some(cx.new(|_| {
            ContextMenu::new(
                entries,
                at,
                move |choice, window, cx| {
                    picked.update(cx, |p, cx| p.menu_pick(choice, window, cx)).ok();
                },
                move |window, cx| {
                    dismissed.update(cx, |p, cx| p.close_menu(window, cx)).ok();
                },
            )
        }));
        cx.notify();
    }

    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            self.editor.read(cx).focus(window);
            cx.notify();
        }
    }

    pub fn menu_pick(&mut self, choice: CodeMenu, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| match choice {
            CodeMenu::Cut => e.cut_selection(cx),
            CodeMenu::Copy => e.copy_selection(cx),
            CodeMenu::Paste => e.paste_clipboard(cx),
            CodeMenu::SelectAll => e.select_everything(cx),
        });
        self.editor.read(cx).focus(window);
    }

    #[cfg(test)]
    pub fn menu_open(&self) -> bool {
        self.menu.is_some()
    }

    fn key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let m = k.modifiers;
        let cmd = (m.control || m.platform) && !m.alt;
        let replace = if MAC { k.key == "f" && m.platform && m.alt } else { k.key == "h" && m.control && !m.alt };
        match k.key.as_str() {
            _ if replace => self.open_find(true, window, cx),
            "f" if cmd && (!m.shift || !MAC) => self.open_find(false, window, cx),
            "g" if cmd && self.find.is_some() => self.step(!m.shift, cx),
            "f3" if self.find.is_some() => self.step(!m.shift, cx),
            "escape" if self.find.is_some() => self.close_find(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for CodePane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bar = self.render_bar(cx);
        div()
            .id("code-pane")
            .key_context("CodeEditor")
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, ev: &MouseDownEvent, _, cx| this.open_menu(ev.position, cx)),
            )
            .relative()
            .size_full()
            .min_h(px(0.0))
            .bg(hsla(self.theme.bg))
            .child(div().size_full().child(self.editor.clone()))
            .children(bar)
            .children(self.menu.clone())
    }
}
