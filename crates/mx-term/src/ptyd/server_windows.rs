use std::io::{self, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use uds_windows::UnixListener;

use super::{ACCEPT_TICK, Daemon, Mirror, Session, UnixStream, finish, lock};
use crate::ptyd::wire::Spawn;
use crate::session::GridSize;
use crate::signals;

const CURSOR_QUERY: &[u8] = b"\x1b[6n";

pub(super) type Clients = std::sync::mpsc::Receiver<io::Result<UnixStream>>;

pub(super) fn clients(listener: UnixListener) -> io::Result<Clients> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new().name("ptyd-accept".into()).spawn(move || {
        loop {
            let accepted = listener.accept().map(|(stream, _)| stream);
            if tx.send(accepted).is_err() {
                return;
            }
        }
    })?;
    Ok(rx)
}

pub(super) fn accept(clients: &Clients) -> io::Result<UnixStream> {
    use std::sync::mpsc::RecvTimeoutError;
    match clients.recv_timeout(ACCEPT_TICK) {
        Ok(accepted) => accepted,
        Err(RecvTimeoutError::Timeout) => Err(ErrorKind::WouldBlock.into()),
        Err(RecvTimeoutError::Disconnected) => Err(io::Error::other("ptyd accept thread ended")),
    }
}

pub(super) fn wait_for_client(_: &Clients) {}

pub(super) fn open_pty(req: &Spawn, size: GridSize) -> anyhow::Result<(crate::conpty::Conpty, u32, crate::conpty::Handle)> {
    let pty = crate::conpty::spawn(&req.program, &req.args, req.cwd.as_ref(), &req.env, size)?;
    let (pid, handle) = (pty.pid, pty.handle.clone());
    Ok((pty, pid, handle))
}

pub(super) fn pump(daemon: &Arc<Daemon>, session: &Arc<Session>, mut pty: crate::conpty::Conpty) {
    let never = AtomicBool::new(false);
    let mut next_check = Instant::now();
    let code = pty
        .run(crate::conpty::Hooks {
            stop: &never,
            deadline: &mut || session.sync_deadline(),
            data: &mut |bytes| session.output(bytes),
            tick: &mut || {
                session.expire_sync();
                session.settle_cursor_queries();
                if Instant::now() >= next_check {
                    next_check = Instant::now() + Duration::from_millis(500);
                    if session.due_to_hibernate() {
                        log::info!("ptyd hibernating idle session {}", session.key);
                        signals::hang_up(session.pid);
                    }
                }
            },
        })
        .flatten();
    finish(daemon, session, code);
}

impl Session {
    pub(super) fn idle_shell(&self) -> bool {
        false
    }

    pub(super) fn redraw(&self) {
        self.settle_cursor_queries();
    }

    pub(super) fn input(&self, bytes: &[u8]) {
        if is_cursor_reply(bytes) {
            let mut m = lock(&self.mirror);
            m.cursor_queries.pending = m.cursor_queries.pending.saturating_sub(1);
        }
        if self.alive() {
            self.pty.write(bytes);
        }
    }

    pub(super) fn resize_pty(&self, size: GridSize) {
        self.pty.resize(size);
    }

    pub(super) fn after_detach(&self) {
        self.settle_cursor_queries();
    }

    // ConPTY holds the child's output until each cursor query is answered; with no app attached, nobody else will.
    pub(super) fn cursor_queries_in(&self, m: &mut Mirror, bytes: &[u8]) {
        let asked = m.cursor_queries.count(bytes);
        if m.client.is_some() {
            m.cursor_queries.pending += asked;
        } else {
            self.answer_cursor(m, asked);
        }
    }

    fn settle_cursor_queries(&self) {
        let mut m = lock(&self.mirror);
        if m.client.is_none() && m.cursor_queries.pending > 0 {
            let pending = std::mem::take(&mut m.cursor_queries.pending);
            self.answer_cursor(&m, pending);
        }
    }

    fn answer_cursor(&self, m: &Mirror, times: usize) {
        let at = m.term.grid().cursor.point;
        let reply = format!("\x1b[{};{}R", at.line.0 + 1, at.column.0 + 1);
        for _ in 0..times {
            self.pty.write(reply.as_bytes());
        }
    }
}

#[derive(Default)]
pub(super) struct CursorQueries {
    pending: usize,
    tail: Vec<u8>,
}

impl CursorQueries {
    fn count(&mut self, bytes: &[u8]) -> usize {
        let mut seen = std::mem::take(&mut self.tail);
        seen.extend_from_slice(bytes);
        let asked = seen.windows(CURSOR_QUERY.len()).filter(|w| *w == CURSOR_QUERY).count();
        let keep = (1..CURSOR_QUERY.len()).rev().find(|n| seen.ends_with(&CURSOR_QUERY[..*n])).unwrap_or(0);
        self.tail = seen[seen.len() - keep..].to_vec();
        asked
    }
}

fn is_cursor_reply(bytes: &[u8]) -> bool {
    let Some(body) = bytes.strip_prefix(b"\x1b[").and_then(|b| b.strip_suffix(b"R")) else { return false };
    let mut parts = body.split(|b| *b == b';');
    let number = |p: Option<&[u8]>| p.is_some_and(|p| !p.is_empty() && p.iter().all(u8::is_ascii_digit));
    number(parts.next()) && number(parts.next()) && parts.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_query_split_over_chunks_is_counted_once_and_two_in_one_chunk_twice() {
        let mut q = CursorQueries::default();
        assert_eq!(q.count(b"x\x1b"), 0);
        assert_eq!(q.count(b"["), 0);
        assert_eq!(q.count(b"6n y"), 1);
        assert_eq!(q.count(b"\x1b[6n\x1b[6n"), 2);
    }

    #[test]
    fn only_a_whole_position_report_counts_as_the_answer() {
        assert!(is_cursor_reply(b"\x1b[12;1R"));
        assert!(!is_cursor_reply(b"\x1b[1;2Rx"));
        assert!(!is_cursor_reply(b"a\x1b[1;2R"));
        assert!(!is_cursor_reply(b"\x1b[R"));
    }
}
