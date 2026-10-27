use std::time::Duration;

use mx_core::ports::{BoxFuture, TermInput, TermKey};

use crate::TermMode;
use crate::session::TerminalSession;
use crate::trust;

pub const KEY_GAP: Duration = Duration::from_millis(60);

pub fn key_text(key: &TermKey, app_cursor: bool) -> String {
    let arrow = |c: char| if app_cursor { format!("\x1bO{c}") } else { format!("\x1b[{c}") };
    match key {
        TermKey::Up => arrow('A'),
        TermKey::Down => arrow('B'),
        TermKey::Right => arrow('C'),
        TermKey::Left => arrow('D'),
        TermKey::Enter => "\r".into(),
        TermKey::Escape => "\x1b".into(),
        TermKey::Tab => "\t".into(),
        TermKey::Space => " ".into(),
        TermKey::Backspace => "\x7f".into(),
        TermKey::Text(text) => text.clone(),
    }
}

impl TerminalSession {
    pub fn screen_text(&self) -> String {
        (0..usize::from(self.size().rows)).map(|row| self.row_text(row) + "\n").collect()
    }

    pub fn asks_trust(&self) -> bool {
        trust::asks_trust(&self.screen_text())
    }

    pub fn press_enter(&self) {
        if !self.is_exited() {
            self.send("\r");
        }
    }

    pub fn send_key(&self, key: &TermKey) {
        self.send(&key_text(key, self.mode().contains(TermMode::APP_CURSOR)));
    }

    pub fn send_keys_paced(&self, keys: Vec<TermKey>, gap: Duration) -> async_channel::Receiver<()> {
        let (tx, rx) = async_channel::bounded(1);
        let me = self.clone();
        std::thread::spawn(move || {
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    std::thread::sleep(gap);
                }
                if me.is_exited() {
                    break;
                }
                me.send_key(key);
            }
            let _ = tx.try_send(());
        });
        rx
    }
}

impl TermInput for TerminalSession {
    fn asks_trust(&self) -> bool {
        TerminalSession::asks_trust(self)
    }

    fn enter(&mut self) {
        self.press_enter();
    }

    fn send_keys(&mut self, keys: Vec<TermKey>) -> BoxFuture<'static, ()> {
        let done = self.send_keys_paced(keys, KEY_GAP);
        Box::pin(async move {
            let _ = done.recv().await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_follow_the_application_cursor_mode() {
        assert_eq!(key_text(&TermKey::Up, false), "\x1b[A");
        assert_eq!(key_text(&TermKey::Up, true), "\x1bOA");
        assert_eq!(key_text(&TermKey::Left, false), "\x1b[D");
        assert_eq!(key_text(&TermKey::Down, true), "\x1bOB");
    }

    #[test]
    fn named_keys_have_their_terminal_bytes() {
        assert_eq!(key_text(&TermKey::Enter, false), "\r");
        assert_eq!(key_text(&TermKey::Escape, false), "\x1b");
        assert_eq!(key_text(&TermKey::Tab, false), "\t");
        assert_eq!(key_text(&TermKey::Space, false), " ");
        assert_eq!(key_text(&TermKey::Backspace, false), "\x7f");
        assert_eq!(key_text(&TermKey::Text("oi".into()), true), "oi");
    }
}
