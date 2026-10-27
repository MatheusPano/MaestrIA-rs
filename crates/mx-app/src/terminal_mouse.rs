use gpui::{
    Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent,
    TouchPhase, Window, px,
};
use mx_term::{MouseAction, MouseButton, SelectionKind, TermMode, clamp_cell, reports_mouse};

use crate::terminal_view::{TerminalView, TerminalViewEvent};

const CLICK_SLOP: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub struct Press {
    at: Point<Pixels>,
    reporting: bool,
    selecting: bool,
    clicks: usize,
    cell: (usize, usize),
}

fn mods(m: &gpui::Modifiers) -> mx_term::Mods {
    mx_term::Mods { ctrl: m.control, alt: m.alt, shift: m.shift, platform: m.platform }
}

impl TerminalView {
    fn cell(&self, position: Point<Pixels>) -> (usize, usize, bool) {
        let st = self.render.borrow();
        let (x, y, right) = st.cell_at(position);
        let (col, row) = clamp_cell(x, y, st.cols, st.rows);
        (col, row, right)
    }

    pub(crate) fn on_left_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        cx.emit(TerminalViewEvent::Focused);
        let (col, row, right) = self.cell(event.position);
        let reporting = reports_mouse(self.session.mode()) && !event.modifiers.shift;
        if reporting {
            self.session.mouse_report(MouseButton::Left, MouseAction::Press, col, row, mods(&event.modifiers));
        } else {
            match event.click_count {
                1 => self.session.clear_selection(),
                2 => self.session.start_selection(col, row, right, SelectionKind::Word),
                _ => self.session.start_selection(col, row, right, SelectionKind::Line),
            }
        }
        self.press = Some(Press {
            at: event.position,
            reporting,
            selecting: event.click_count > 1,
            clicks: event.click_count,
            cell: (col, row),
        });
        cx.notify();
    }

    pub(crate) fn on_right_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        cx.emit(TerminalViewEvent::Focused);
        let (col, row, _) = self.cell(event.position);
        let link = self.session.link_at(col, row, self.cwd.as_deref());
        self.open_menu(event.position, link, window, cx);
    }

    pub(crate) fn on_mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut press) = self.press else { return };
        if event.pressed_button != Some(gpui::MouseButton::Left) {
            return;
        }
        let (col, row, right) = self.cell(event.position);
        if press.reporting {
            if press.cell != (col, row) {
                press.cell = (col, row);
                self.press = Some(press);
                self.session.mouse_report(MouseButton::Left, MouseAction::Drag, col, row, mods(&event.modifiers));
            }
            return;
        }
        if !press.selecting {
            let moved = (event.position - press.at).magnitude() as f32;
            if moved <= CLICK_SLOP {
                return;
            }
            let (c0, r0, right0) = self.cell(press.at);
            self.session.start_selection(c0, r0, right0, SelectionKind::Cells);
            press.selecting = true;
            self.press = Some(press);
        }
        self.session.update_selection(col, row, right);
        cx.notify();
    }

    pub(crate) fn on_left_up(&mut self, event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(press) = self.press.take() else { return };
        let (col, row, _) = self.cell(event.position);
        if press.reporting {
            self.session.mouse_report(MouseButton::Left, MouseAction::Release, col, row, mods(&event.modifiers));
            return;
        }
        let moved = (event.position - press.at).magnitude() as f32;
        if press.selecting || press.clicks > 1 || moved > CLICK_SLOP {
            return;
        }
        if let Some(href) = self.session.link_at(col, row, self.cwd.as_deref()) {
            self.follow(&href, cx);
        }
    }

    pub(crate) fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let line = self.render.borrow().cell().height;
        if matches!(event.touch_phase, TouchPhase::Started) {
            self.wheel.reset();
        }
        let dy = f32::from(event.delta.pixel_delta(line).y);
        let mode = self.session.mode();
        if mode.contains(TermMode::ALT_SCREEN) {
            let notches = self.wheel.notches(dy, f32::from(line.max(px(1.))));
            if notches == 0 {
                return;
            }
            let (col, row, _) = self.cell(event.position);
            for _ in 0..notches.unsigned_abs() {
                self.session.wheel(notches > 0, col, row);
            }
        } else {
            let notches = self.wheel.notches(dy, f32::from(line.max(px(1.))));
            if notches != 0 {
                self.session.scroll(notches);
                cx.notify();
            }
        }
        cx.stop_propagation();
    }
}
