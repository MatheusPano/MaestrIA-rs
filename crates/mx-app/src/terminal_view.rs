use gpui::{
    ClipboardItem, Context, FocusHandle, Focusable, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, ParentElement, Render, ScrollWheelEvent, Styled, Window, div, prelude::*,
};
use mx_term::{GridSize, Mods, SpawnOptions, TermEvent, TermMode, TerminalSession, encode_key};
use mx_theme::TerminalPalette;

use crate::terminal_element::{TerminalElement, cell_metrics};

pub struct TerminalView {
    session: TerminalSession,
    focus: FocusHandle,
    scroll_remainder: f32,
}

impl TerminalView {
    pub fn new(
        options: SpawnOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Self> {
        let initial = GridSize {
            cols: 80,
            rows: 24,
            cell_width: 8,
            cell_height: 18,
        };
        let (session, events) =
            TerminalSession::spawn(options, initial, TerminalPalette::default())?;
        log::info!("spawned shell pid {}", session.child_pid());

        cx.spawn_in(window, async move |this, cx| {
            while let Ok(first) = events.recv().await {
                let mut batch = vec![first];
                while let Ok(more) = events.try_recv() {
                    batch.push(more);
                }
                let alive =
                    this.update_in(cx, |view, window, cx| view.on_events(batch, window, cx));
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        Ok(Self {
            session,
            focus: cx.focus_handle(),
            scroll_remainder: 0.0,
        })
    }

    pub fn session(&self) -> &TerminalSession {
        &self.session
    }

    pub fn focus(&self) -> &FocusHandle {
        &self.focus
    }

    fn on_events(&mut self, events: Vec<TermEvent>, window: &mut Window, cx: &mut Context<Self>) {
        for event in events {
            match event {
                TermEvent::Wakeup | TermEvent::Bell => {}
                TermEvent::Title(title) => window.set_window_title(&title),
                TermEvent::ResetTitle => window.set_window_title("Maestria"),
                TermEvent::ClipboardStore(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text))
                }
                TermEvent::Exited(code) => {
                    log::info!("shell exited with {code:?}");
                    cx.quit();
                    return;
                }
            }
        }
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let mods = Mods {
            ctrl: ks.modifiers.control,
            alt: ks.modifiers.alt,
            shift: ks.modifiers.shift,
            platform: ks.modifiers.platform,
        };
        if mods.ctrl && mods.shift && ks.key == "v" {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.paste(&text);
            }
            cx.stop_propagation();
            return;
        }
        let app_cursor = self.session.mode().contains(TermMode::APP_CURSOR);
        if let Some(bytes) = encode_key(&ks.key, ks.key_char.as_deref(), mods, app_cursor) {
            self.session.write(bytes.into_owned());
            cx.stop_propagation();
        }
    }

    fn paste(&self, text: &str) {
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if self.session.mode().contains(TermMode::BRACKETED_PASTE) {
            self.session
                .write(format!("\x1b[200~{text}\x1b[201~").into_bytes());
        } else {
            self.session.write(text.into_bytes());
        }
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let line_height = cell_metrics(window).height;
        let lines = self.scroll_remainder + event.delta.pixel_delta(line_height).y / line_height;
        let whole = lines.trunc();
        self.scroll_remainder = lines - whole;
        if whole != 0.0 {
            self.session.scroll(whole as i32);
            cx.notify();
        }
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
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _: &MouseDownEvent, window, _| window.focus(&view.focus)),
            )
            .size_full()
            .child(TerminalElement::new(cx.entity()))
    }
}
