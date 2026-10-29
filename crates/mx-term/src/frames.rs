use std::time::{Duration, Instant};

pub const FRAME_PATIENCE: Duration = Duration::from_millis(40);

const HIDE: &[u8] = b"\x1b[?25l";
const SHOW: &[u8] = b"\x1b[?25h";

/// Holds a TUI frame until it is whole, so the screen is never painted in the
/// middle of a redraw.
///
/// The macOS pty hands out at most 1024 bytes per read, and Claude Code (Ink)
/// opens every frame hiding the cursor (`ESC[?25l`) and closes it showing it
/// again (`ESC[?25h`), without synchronized output. A `?25l` with no `?25h`
/// after it is an open frame, and from it on the output waits — for at most
/// [`FRAME_PATIENCE`], since a program that hides the cursor for good (`htop`)
/// still needs a screen.
#[derive(Debug, Default)]
pub struct VtFrameGate {
    held: Vec<u8>,
    deadline: Option<Instant>,
}

impl VtFrameGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends to `out` what can be written now; the open frame stays held.
    pub fn feed(&mut self, chunk: &[u8], now: Instant, out: &mut Vec<u8>) {
        if chunk.is_empty() {
            return;
        }
        // The held part starts at its only `?25l` and has no `?25h`: only the
        // tail where a new marker can end needs searching.
        let from = self.held.len().saturating_sub(HIDE.len() - 1);
        self.held.extend_from_slice(chunk);
        let open = rfind(&self.held[from..], HIDE).map(|i| from + i);
        let open = match open {
            Some(i) => i,
            None if from > 0 => 0,
            None => return self.release(out),
        };
        let shown_from = open.max(from);
        if rfind(&self.held[shown_from..], SHOW).is_some() {
            return self.release(out);
        }
        out.extend_from_slice(&self.held[..open]);
        self.held.drain(..open);
        self.deadline.get_or_insert(now + FRAME_PATIENCE);
    }

    /// Releases whatever is held: the patience ran out, or the process left.
    pub fn flush(&mut self, out: &mut Vec<u8>) {
        self.release(out);
    }

    /// [`flush`](Self::flush), once `now` has reached the deadline.
    pub fn expire(&mut self, now: Instant, out: &mut Vec<u8>) {
        if self.deadline.is_some_and(|d| now >= d) {
            self.release(out);
        }
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    fn release(&mut self, out: &mut Vec<u8>) {
        self.deadline = None;
        out.append(&mut self.held);
    }
}

fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(g: &mut VtFrameGate, chunk: &str, now: Instant) -> String {
        let mut out = Vec::new();
        g.feed(chunk.as_bytes(), now, &mut out);
        String::from_utf8(out).unwrap()
    }

    fn flushed(g: &mut VtFrameGate) -> String {
        let mut out = Vec::new();
        g.flush(&mut out);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn text_without_a_frame_passes_at_once() {
        let mut g = VtFrameGate::new();
        assert_eq!(feed(&mut g, "ola\r\n", Instant::now()), "ola\r\n");
        assert_eq!(g.deadline(), None);
    }

    #[test]
    fn a_frame_cut_in_two_comes_out_whole() {
        let mut g = VtFrameGate::new();
        let now = Instant::now();
        assert_eq!(feed(&mut g, "antes\x1b[?25l\x1b[Hmetade", now), "antes");
        assert_eq!(
            feed(&mut g, " e o resto\x1b[37;6H\x1b[?25h", now),
            "\x1b[?25l\x1b[Hmetade e o resto\x1b[37;6H\x1b[?25h"
        );
        assert_eq!(g.deadline(), None);
    }

    #[test]
    fn a_show_split_between_chunks_still_closes_the_frame() {
        let mut g = VtFrameGate::new();
        let now = Instant::now();
        assert_eq!(feed(&mut g, "\x1b[?25lquadro\x1b[?25", now), "");
        assert_eq!(feed(&mut g, "h", now), "\x1b[?25lquadro\x1b[?25h");
    }

    #[test]
    fn a_hide_split_between_chunks_is_found_in_the_joined_text() {
        let mut g = VtFrameGate::new();
        let now = Instant::now();
        assert_eq!(feed(&mut g, "antes\x1b[?25lx", now), "antes");
        assert_eq!(feed(&mut g, "y\x1b[?2", now), "");
        assert_eq!(feed(&mut g, "5lz", now), "\x1b[?25lxy");
        assert_eq!(feed(&mut g, "\x1b[?25h", now), "\x1b[?25lz\x1b[?25h");
    }

    #[test]
    fn a_whole_frame_in_one_chunk_does_not_wait() {
        let mut g = VtFrameGate::new();
        assert_eq!(feed(&mut g, "\x1b[?25lola\x1b[?25h", Instant::now()), "\x1b[?25lola\x1b[?25h");
        assert_eq!(g.deadline(), None);
    }

    #[test]
    fn only_the_last_hide_counts() {
        let mut g = VtFrameGate::new();
        let now = Instant::now();
        assert_eq!(feed(&mut g, "\x1b[?25la\x1b[?25hb\x1b[?25lc", now), "\x1b[?25la\x1b[?25hb");
        assert_eq!(flushed(&mut g), "\x1b[?25lc");
    }

    #[test]
    fn multibyte_text_split_across_chunks_is_untouched() {
        let mut g = VtFrameGate::new();
        let now = Instant::now();
        let bytes = "\x1b[?25lação\x1b[?25h".as_bytes();
        let mut out = Vec::new();
        g.feed(&bytes[..9], now, &mut out);
        assert!(out.is_empty());
        g.feed(&bytes[9..], now, &mut out);
        assert_eq!(out, bytes);
    }

    #[test]
    fn whoever_hides_the_cursor_for_good_still_gets_the_screen() {
        let mut g = VtFrameGate::new();
        let start = Instant::now();
        assert_eq!(feed(&mut g, "\x1b[?25lhtop", start), "");
        assert_eq!(g.deadline(), Some(start + FRAME_PATIENCE));
        // A later chunk does not push the deadline back.
        assert_eq!(feed(&mut g, " mais", start + Duration::from_millis(30)), "");
        assert_eq!(g.deadline(), Some(start + FRAME_PATIENCE));
        let mut out = Vec::new();
        g.expire(start + Duration::from_millis(39), &mut out);
        assert!(out.is_empty());
        g.expire(start + Duration::from_millis(50), &mut out);
        assert_eq!(out, b"\x1b[?25lhtop mais");
        assert_eq!(g.deadline(), None);
    }

    #[test]
    fn a_closed_frame_cancels_the_timer() {
        let mut g = VtFrameGate::new();
        let now = Instant::now();
        feed(&mut g, "\x1b[?25la", now);
        assert!(g.deadline().is_some());
        feed(&mut g, "b\x1b[?25h", now);
        assert_eq!(g.deadline(), None);
    }

    #[test]
    fn flush_releases_what_was_held() {
        let mut g = VtFrameGate::new();
        assert_eq!(feed(&mut g, "\x1b[?25lultimo", Instant::now()), "");
        assert_eq!(flushed(&mut g), "\x1b[?25lultimo");
        assert_eq!(g.deadline(), None);
        assert_eq!(flushed(&mut g), "");
    }
}
