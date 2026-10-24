use std::collections::VecDeque;
use std::time::{Duration, Instant};

const ECHO_WINDOW: Duration = Duration::from_secs(2);
const ECHO_CAP: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOrigin {
    Key,
    Commit,
    CommitKey,
}

#[derive(Debug, Default)]
pub struct TextInputGuard {
    unspent: bool,
    last: Option<String>,
    composing: bool,
    held: Vec<String>,
    echoes: VecDeque<(String, Instant)>,
    commit_echoes: VecDeque<(String, Instant)>,
}

impl TextInputGuard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn key_down(&mut self, key: &str) {
        self.unspent = true;
        if !self.held.iter().any(|k| k == key) {
            self.held.push(key.to_owned());
        }
    }

    pub fn key_up(&mut self, key: &str) {
        match self.held.iter().position(|k| k == key) {
            Some(i) => {
                self.held.swap_remove(i);
            }
            None => self.unspent = true,
        }
    }

    pub fn preedit(&mut self, text: &str) {
        self.composing = !text.is_empty();
        if self.composing {
            self.echoes.clear();
            self.commit_echoes.clear();
        }
    }

    pub fn forget_last(&mut self) {
        self.last = None;
    }

    pub fn composing(&self) -> bool {
        self.composing
    }

    pub fn accept(&mut self, text: &str, origin: TextOrigin, now: Instant) -> bool {
        if text.is_empty() {
            return false;
        }
        match origin {
            TextOrigin::Key => {
                self.composing = false;
                if take_echo(&mut self.commit_echoes, text, now) {
                    return false;
                }
                remember(&mut self.echoes, text, now);
            }
            TextOrigin::Commit if self.composing => self.composing = false,
            TextOrigin::Commit => {
                if take_echo(&mut self.echoes, text, now) || !self.unspent && self.last.as_deref() == Some(text) {
                    return false;
                }
            }
            TextOrigin::CommitKey => {
                self.composing = false;
                if take_echo(&mut self.echoes, text, now) {
                    return false;
                }
                remember(&mut self.commit_echoes, text, now);
            }
        }
        self.unspent = false;
        self.last = Some(text.to_owned());
        true
    }
}

fn remember(echoes: &mut VecDeque<(String, Instant)>, text: &str, now: Instant) {
    if echoes.len() == ECHO_CAP {
        echoes.pop_front();
    }
    echoes.push_back((text.to_owned(), now));
}

fn take_echo(echoes: &mut VecDeque<(String, Instant)>, text: &str, now: Instant) -> bool {
    echoes.retain(|(_, at)| now.saturating_duration_since(*at) <= ECHO_WINDOW);
    match echoes.iter().position(|(t, _)| t == text) {
        Some(i) => {
            echoes.remove(i);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Pane {
        guard: TextInputGuard,
        written: Vec<String>,
        now: Instant,
    }

    impl Pane {
        fn new() -> Self {
            Self { guard: TextInputGuard::new(), written: Vec::new(), now: Instant::now() }
        }

        fn down(&mut self, key: &str) -> &mut Self {
            self.guard.key_down(key);
            self
        }

        fn up(&mut self, key: &str) -> &mut Self {
            self.guard.key_up(key);
            self
        }

        fn preedit(&mut self, text: &str) -> &mut Self {
            self.guard.preedit(text);
            self
        }

        fn input(&mut self, text: &str, origin: TextOrigin) -> &mut Self {
            if self.guard.accept(text, origin, self.now) {
                self.written.push(text.to_owned());
            }
            self
        }

        fn commit(&mut self, text: &str) -> &mut Self {
            self.input(text, TextOrigin::Commit)
        }

        fn key_text(&mut self, text: &str) -> &mut Self {
            self.input(text, TextOrigin::Key)
        }

        fn later(&mut self, by: Duration) -> &mut Self {
            self.now += by;
            self
        }
    }

    #[test]
    fn dead_key_with_preedit_writes_nothing() {
        let mut p = Pane::new();
        p.down("dead_acute").preedit("´").up("dead_acute");
        assert!(p.written.is_empty());
    }

    #[test]
    fn dead_key_then_e_writes_one_e_acute() {
        let mut p = Pane::new();
        p.down("dead_acute").preedit("´").up("dead_acute");
        p.down("e").commit("é").up("e");
        assert_eq!(p.written, ["é"]);
    }

    #[test]
    fn cedilla_key_commit_writes_once() {
        let mut p = Pane::new();
        p.down(";").commit("ç").up(";");
        assert_eq!(p.written, ["ç"]);
    }

    #[test]
    fn repeated_ibus_commit_writes_once() {
        let mut p = Pane::new();
        p.preedit("´").down("a").up("a");
        p.commit("á").commit("á").commit("á");
        assert_eq!(p.written, ["á"]);
    }

    #[test]
    fn composing_twice_with_full_key_sequences_writes_twice() {
        let mut p = Pane::new();
        for _ in 0..2 {
            p.down("dead_acute").preedit("´").up("dead_acute");
            p.down("a").commit("á").up("a");
        }
        assert_eq!(p.written, ["á", "á"]);
    }

    #[test]
    fn held_key_repeats_each_write() {
        let mut p = Pane::new();
        p.down(";").commit("ç");
        for _ in 0..3 {
            p.down(";").commit("ç");
        }
        assert_eq!(p.written, ["ç", "ç", "ç", "ç"]);
    }

    #[test]
    fn pt_br_dead_tilde_composes_nao() {
        let mut p = Pane::new();
        p.down("n").key_text("n").up("n");
        p.down("dead_tilde").preedit("~").up("dead_tilde");
        p.down("a").commit("ã").up("a");
        p.down("o").key_text("o").up("o");
        assert_eq!(p.written.concat(), "não");
    }

    #[test]
    fn key_char_and_commit_for_same_key_write_once() {
        let mut p = Pane::new();
        p.down("a").key_text("a").commit("a").up("a");
        assert_eq!(p.written, ["a"]);
    }

    #[test]
    fn late_commit_echo_after_release_is_dropped() {
        let mut p = Pane::new();
        p.down("a").key_text("a").up("a").commit("a");
        assert_eq!(p.written, ["a"]);
    }

    #[test]
    fn late_commit_echo_after_next_key_is_dropped() {
        let mut p = Pane::new();
        p.down("a").key_text("a").down("b").key_text("b").commit("a").up("a").up("b");
        assert_eq!(p.written.concat(), "ab");
    }

    #[test]
    fn fast_double_letters_from_keys_all_written() {
        let mut p = Pane::new();
        for _ in 0..3 {
            p.down("l").key_text("l").up("l");
        }
        assert_eq!(p.written.concat(), "lll");
    }

    #[test]
    fn commit_only_engine_repeated_letters_all_written() {
        let mut p = Pane::new();
        for _ in 0..3 {
            p.commit("x").up("x");
        }
        assert_eq!(p.written.concat(), "xxx");
    }

    #[test]
    fn stale_echo_does_not_eat_a_much_later_commit() {
        let mut p = Pane::new();
        p.down("a").key_text("a").up("a");
        p.later(Duration::from_secs(5)).down("x").commit("a");
        assert_eq!(p.written, ["a", "a"]);
    }

    #[test]
    fn empty_text_is_dropped_without_spending_the_key() {
        let mut p = Pane::new();
        p.down("a").key_text("a").up("a").down("a").commit("").key_text("a");
        assert_eq!(p.written, ["a", "a"]);
    }

    #[test]
    fn wayland_one_byte_commit_after_the_real_key_is_dropped() {
        let mut p = Pane::new();
        p.down("a").key_text("a").input("a", TextOrigin::CommitKey).up("a");
        assert_eq!(p.written, ["a"]);
    }

    #[test]
    fn wayland_one_byte_commit_after_release_is_dropped() {
        let mut p = Pane::new();
        p.down("a").key_text("a").up("a").input("a", TextOrigin::CommitKey);
        assert_eq!(p.written, ["a"]);
    }

    #[test]
    fn wayland_one_byte_commit_before_the_real_key_writes_once() {
        let mut p = Pane::new();
        p.input("a", TextOrigin::CommitKey).down("a").key_text("a").up("a");
        assert_eq!(p.written, ["a"]);
    }

    #[test]
    fn wayland_commit_only_typing_keeps_double_letters() {
        let mut p = Pane::new();
        for c in ["h", "e", "l", "l", "o"] {
            p.input(c, TextOrigin::CommitKey);
        }
        assert_eq!(p.written.concat(), "hello");
    }

    #[test]
    fn wayland_mixed_keys_and_commits_write_each_keystroke_once() {
        let mut p = Pane::new();
        p.down("l").key_text("l").input("l", TextOrigin::CommitKey).up("l");
        p.down("l").key_text("l").up("l").input("l", TextOrigin::CommitKey);
        p.input("l", TextOrigin::CommitKey);
        assert_eq!(p.written.concat(), "lll");
    }

    #[test]
    fn app_input_clears_last_so_same_text_passes() {
        let mut p = Pane::new();
        p.commit("x");
        p.guard.forget_last();
        p.commit("x");
        assert_eq!(p.written, ["x", "x"]);
    }
}
