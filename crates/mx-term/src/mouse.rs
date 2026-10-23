use alacritty_terminal::term::TermMode;

use crate::keys::Mods;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    Drag,
}

pub fn reports_mouse(mode: TermMode) -> bool {
    mode.intersects(TermMode::MOUSE_MODE)
}

pub fn wheel_bytes(up: bool, col: usize, row: usize, mode: TermMode) -> Vec<u8> {
    if !reports_mouse(mode) {
        let app = mode.contains(TermMode::APP_CURSOR);
        return match (up, app) {
            (true, false) => b"\x1b[A".to_vec(),
            (true, true) => b"\x1bOA".to_vec(),
            (false, false) => b"\x1b[B".to_vec(),
            (false, true) => b"\x1bOB".to_vec(),
        };
    }
    encode(if up { 64 } else { 65 }, true, col + 1, row + 1, mode)
}

pub fn button_report(
    button: MouseButton,
    action: MouseAction,
    col: usize,
    row: usize,
    mods: Mods,
    mode: TermMode,
) -> Option<Vec<u8>> {
    let drag = action == MouseAction::Drag;
    if !reports_mouse(mode) || drag && !mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) {
        return None;
    }
    let base = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    let sgr = mode.contains(TermMode::SGR_MOUSE);
    let mut code = if action == MouseAction::Release && !sgr { 3 } else { base };
    if drag {
        code += 32;
    }
    code += 4 * u8::from(mods.shift) + 8 * u8::from(mods.alt) + 16 * u8::from(mods.ctrl);
    Some(encode(code, action != MouseAction::Release, col + 1, row + 1, mode))
}

fn encode(code: u8, pressed: bool, x: usize, y: usize, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::SGR_MOUSE) {
        let fin = if pressed { 'M' } else { 'm' };
        return format!("\x1b[<{code};{x};{y}{fin}").into_bytes();
    }
    let mut out = b"\x1b[M".to_vec();
    let utf8 = mode.contains(TermMode::UTF8_MOUSE);
    for v in [usize::from(code), x, y] {
        let v = 32 + v;
        match char::from_u32(v as u32) {
            Some(c) if utf8 && v < 2048 => {
                let mut buf = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            _ => out.push(v.min(255) as u8),
        }
    }
    out
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WheelAccumulator {
    pending: f32,
}

impl WheelAccumulator {
    pub fn reset(&mut self) {
        self.pending = 0.0;
    }

    pub fn notches(&mut self, delta_up: f32, line_height: f32) -> i32 {
        if line_height <= 0.0 {
            return 0;
        }
        self.pending += delta_up;
        let notches = (self.pending / line_height).trunc();
        self.pending -= notches * line_height;
        notches as i32
    }
}

pub fn clamp_cell(col: f32, row: f32, cols: usize, rows: usize) -> (usize, usize) {
    let clamp = |v: f32, n: usize| (v.max(0.0) as usize).min(n.saturating_sub(1));
    (clamp(col, cols), clamp(row, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_mouse() -> TermMode {
        TermMode::ALT_SCREEN
            | TermMode::MOUSE_REPORT_CLICK
            | TermMode::MOUSE_DRAG
            | TermMode::MOUSE_MOTION
            | TermMode::SGR_MOUSE
    }

    #[test]
    fn wheel_up_then_down_at_origin_reports_64_and_65() {
        assert_eq!(wheel_bytes(true, 0, 0, full_mouse()), b"\x1b[<64;1;1M");
        assert_eq!(wheel_bytes(false, 0, 0, full_mouse()), b"\x1b[<65;1;1M");
    }

    #[test]
    fn wheel_down_at_col_41_row_7_is_one_based() {
        assert_eq!(wheel_bytes(false, 41, 7, full_mouse()), b"\x1b[<65;42;8M");
    }

    #[test]
    fn wheel_without_mouse_reporting_sends_arrow() {
        assert_eq!(wheel_bytes(true, 3, 3, TermMode::ALT_SCREEN), b"\x1b[A");
        let app = TermMode::ALT_SCREEN | TermMode::APP_CURSOR;
        assert_eq!(wheel_bytes(true, 3, 3, app), b"\x1bOA");
        assert_eq!(wheel_bytes(false, 3, 3, app), b"\x1bOB");
    }

    #[test]
    fn wheel_normal_encoding() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        assert_eq!(wheel_bytes(true, 0, 0, mode), vec![0x1b, b'[', b'M', 96, 33, 33]);
    }

    #[test]
    fn left_button_down_is_reported() {
        let r = button_report(MouseButton::Left, MouseAction::Press, 2, 0, Mods::default(), full_mouse());
        assert_eq!(r.unwrap(), b"\x1b[<0;3;1M");
        let r = button_report(MouseButton::Left, MouseAction::Release, 2, 0, Mods::default(), full_mouse());
        assert_eq!(r.unwrap(), b"\x1b[<0;3;1m");
    }

    #[test]
    fn no_report_without_mouse_mode() {
        let r = button_report(MouseButton::Left, MouseAction::Press, 0, 0, Mods::default(), TermMode::empty());
        assert!(r.is_none());
    }

    #[test]
    fn drag_needs_drag_mode() {
        let click = TermMode::MOUSE_REPORT_CLICK;
        assert!(button_report(MouseButton::Left, MouseAction::Drag, 0, 0, Mods::default(), click).is_none());
        let r = button_report(MouseButton::Left, MouseAction::Drag, 0, 0, Mods::default(), full_mouse());
        assert_eq!(r.unwrap(), b"\x1b[<32;1;1M");
    }

    #[test]
    fn accumulator_truncates_toward_zero() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.notches(10.0, 16.0), 0);
        assert_eq!(acc.notches(10.0, 16.0), 1);
        assert_eq!(acc.notches(-40.0, 16.0), -2);
        acc.reset();
        assert_eq!(acc.notches(-15.0, 16.0), 0);
    }

    #[test]
    fn clamp_cell_bounds() {
        assert_eq!(clamp_cell(-3.0, 100.0, 80, 24), (0, 23));
        assert_eq!(clamp_cell(5.7, 2.2, 80, 24), (5, 2));
    }
}
