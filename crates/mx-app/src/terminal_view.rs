use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, IntoElement, KeyDownEvent,
    KeyUpEvent, MouseButton, ParentElement, Pixels, Point, Render, Styled, Window, div,
    prelude::*,
};
use mx_term::{
    GridSize, LinkFollow, Mods, SpawnOptions, TermEvent, TermMode, TerminalSession,
    WheelAccumulator, encode_key, is_slot_chord, is_text_key,
};
use mx_theme::TerminalPalette;

use crate::terminal_element::{RenderState, SharedRender, TerminalElement, Typography};
use crate::terminal_mouse::Press;

#[derive(Clone, Debug, PartialEq)]
pub enum TerminalViewEvent {
    Focused,
    Link(LinkFollow),
    ContextMenu { position: Point<Pixels>, can_copy: bool },
    Exited(Option<i32>),
    Title(String),
}

pub struct TerminalView {
    pub(crate) session: TerminalSession,
    pub(crate) focus: FocusHandle,
    pub(crate) render: SharedRender,
    pub(crate) wheel: WheelAccumulator,
    pub(crate) press: Option<Press>,
    pub(crate) pending_key: Option<String>,
    pub(crate) claude: bool,
    pub(crate) cwd: Option<String>,
}

impl EventEmitter<TerminalViewEvent> for TerminalView {}

fn mods_of(m: &gpui::Modifiers) -> Mods {
    Mods { ctrl: m.control, alt: m.alt, shift: m.shift, platform: m.platform }
}

impl TerminalView {
    pub fn new(options: SpawnOptions, window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let cwd = options.cwd.as_ref().map(|p| p.display().to_string());
        let palette = TerminalPalette::default();
        let session = TerminalSession::spawn(options, GridSize::DEFAULT, palette.clone())?;
        let mut view = Self::for_session(session, &palette, window, cx);
        view.cwd = cwd;
        Ok(view)
    }

    pub fn for_session(
        session: TerminalSession,
        palette: &TerminalPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let events = session.events();
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(first) = events.recv().await {
                let mut batch = vec![first];
                while let Ok(more) = events.try_recv() {
                    batch.push(more);
                }
                if this.update_in(cx, |view, _, cx| view.on_events(batch, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            session,
            focus: cx.focus_handle(),
            render: Rc::new(RefCell::new(RenderState::new(Typography::default(), palette.selection))),
            wheel: WheelAccumulator::default(),
            press: None,
            pending_key: None,
            claude: false,
            cwd: None,
        }
    }

    pub fn session(&self) -> &TerminalSession {
        &self.session
    }

    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    pub fn render(&self) -> &SharedRender {
        &self.render
    }

    pub fn set_claude(&mut self, claude: bool) {
        self.claude = claude;
    }

    pub fn set_cwd(&mut self, cwd: Option<String>) {
        self.cwd = cwd;
    }

    pub fn set_typography(&mut self, size: f32, line: f32, cx: &mut Context<Self>) {
        let t = Typography { size: size.clamp(9.0, 24.0), line: line.clamp(1.0, 1.8) };
        let mut st = self.render.borrow_mut();
        if st.typography != t {
            st.typography = t;
            drop(st);
            cx.notify();
        }
    }

    pub fn set_palette(&mut self, palette: &TerminalPalette, cx: &mut Context<Self>) {
        self.render.borrow_mut().selection = palette.selection;
        self.session.set_palette(palette.clone());
        cx.notify();
    }

    fn on_events(&mut self, events: Vec<TermEvent>, cx: &mut Context<Self>) {
        let mut redraw = false;
        for event in events {
            match event {
                TermEvent::Wakeup => redraw = true,
                TermEvent::Bell | TermEvent::ResetTitle => {}
                TermEvent::Title(title) => cx.emit(TerminalViewEvent::Title(title)),
                TermEvent::ClipboardStore(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
                TermEvent::Exited(code) => {
                    redraw = true;
                    cx.emit(TerminalViewEvent::Exited(code));
                }
            }
        }
        if redraw {
            let mut st = self.render.borrow_mut();
            if st.painted {
                st.painted = false;
                drop(st);
                cx.notify();
            }
        }
    }

    pub fn copy_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.session.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        let text = cx.read_from_clipboard().and_then(|item| item.text());
        self.session.paste_clipboard(text.as_deref(), self.claude);
        cx.notify();
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.session.select_all();
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let mods = mods_of(&ks.modifiers);
        self.pending_key = None;
        self.session.key_down(&ks.key);
        let only = |ctrl: bool, shift: bool| mods.ctrl == ctrl && mods.shift == shift && !mods.alt && !mods.platform;
        let handled = match ks.key.as_str() {
            "enter" if only(false, true) => {
                self.session.newline();
                true
            }
            "v" if only(true, true) || only(true, false) => {
                self.paste_clipboard(cx);
                true
            }
            "insert" if only(false, true) => {
                self.paste_clipboard(cx);
                true
            }
            "c" if only(true, true) => {
                self.copy_selection(cx);
                true
            }
            "a" if only(true, true) => {
                self.select_all(cx);
                true
            }
            _ => false,
        };
        if handled {
            cx.stop_propagation();
            return;
        }
        if is_slot_chord(&ks.key, mods) {
            return;
        }
        if is_text_key(&ks.key, ks.key_char.as_deref(), mods) {
            self.pending_key = ks.key_char.clone();
            return;
        }
        let app_cursor = self.session.mode().contains(TermMode::APP_CURSOR);
        if let Some(bytes) = encode_key(&ks.key, ks.key_char.as_deref(), mods, app_cursor) {
            self.session.write(&bytes);
            cx.stop_propagation();
        }
    }

    fn on_key_up(&mut self, event: &KeyUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        self.session.key_up(&event.keystroke.key);
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("terminal")
            .key_context("Terminal")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_key_up(cx.listener(Self::on_key_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_left_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_left_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_left_up))
            .size_full()
            .child(TerminalElement::new(cx.entity()))
    }
}
