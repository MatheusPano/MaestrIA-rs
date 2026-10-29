use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite, Pty};
use alacritty_terminal::vte::ansi::{Processor, Rgb as AnsiRgb};
use mx_theme::TerminalPalette;
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::events::Fanout;
use crate::frames::VtFrameGate;
use crate::scrubber::VtScrubber;
use crate::session::{GridSize, TermEvent};
use crate::sticky::StickyClock;

const READ_CHUNK: usize = 0x10000;
const MAX_LOCKED_READ: usize = 0x40000;
const CHILD_CHECK: Duration = Duration::from_millis(500);
const HUP_RECHECK: Duration = Duration::from_millis(10);

pub(crate) struct Writer {
    generation: u64,
    file: File,
    pending: Vec<u8>,
    waker: UnixStream,
}

impl Writer {
    fn push(&mut self, bytes: &[u8]) {
        let mut rest = bytes;
        if self.pending.is_empty() {
            rest = write_some(&self.file, rest);
        }
        if !rest.is_empty() {
            self.pending.extend_from_slice(rest);
            let _ = (&self.waker).write(&[1]);
        }
    }

    fn flush(&mut self) {
        let left = write_some(&self.file, &self.pending).len();
        let done = self.pending.len() - left;
        self.pending.drain(..done);
    }
}

fn write_some<'a>(mut file: &File, mut bytes: &'a [u8]) -> &'a [u8] {
    while !bytes.is_empty() {
        match file.write(bytes) {
            Ok(0) => break,
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(_) => return &[],
        }
    }
    bytes
}

pub(crate) struct Port {
    pub fanout: Fanout,
    pub writer: Mutex<Option<Writer>>,
    pub size: Mutex<GridSize>,
    pub palette: Mutex<Arc<TerminalPalette>>,
    pub sticky: Arc<StickyClock>,
}

impl Port {
    pub fn write(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.writer.lock()
            && let Some(writer) = guard.as_mut()
        {
            writer.push(bytes);
        }
    }

    pub fn wake(&self) {
        self.fanout.wake();
    }

    pub fn emit(&self, event: TermEvent) {
        self.fanout.emit(event);
    }

    pub fn resize_pty(&self, size: GridSize) {
        if let Ok(guard) = self.writer.lock()
            && let Some(writer) = guard.as_ref()
        {
            let ws = rustix::termios::Winsize {
                ws_row: size.rows,
                ws_col: size.cols,
                ws_xpixel: size.cols.saturating_mul(size.cell_width),
                ws_ypixel: size.rows.saturating_mul(size.cell_height),
            };
            let _ = rustix::termios::tcsetwinsize(writer.file.as_fd(), ws);
        }
    }
}

#[derive(Clone)]
pub(crate) struct Proxy(pub Arc<Port>);

impl EventListener for Proxy {
    fn send_event(&self, event: Event) {
        let port = &self.0;
        match event {
            Event::Title(t) => port.emit(TermEvent::Title(t)),
            Event::ResetTitle => port.emit(TermEvent::ResetTitle),
            Event::Bell => port.emit(TermEvent::Bell),
            Event::ClipboardStore(_, text) => port.emit(TermEvent::ClipboardStore(text)),
            Event::PtyWrite(text) => port.write(text.as_bytes()),
            Event::TextAreaSizeRequest(format) => {
                let size = port.size.lock().map(|s| *s).ok();
                if let Some(s) = size {
                    port.write(
                        format(WindowSize {
                            num_lines: s.rows,
                            num_cols: s.cols,
                            cell_width: s.cell_width,
                            cell_height: s.cell_height,
                        })
                        .as_bytes(),
                    );
                }
            }
            Event::ColorRequest(index, format) => {
                let Ok(palette) = port.palette.lock().map(|p| p.clone()) else {
                    return;
                };
                let c = match index {
                    0..=255 => palette.indexed(index as u8),
                    257 => palette.background,
                    258 => palette.cursor,
                    _ => palette.foreground,
                };
                port.write(format(AnsiRgb { r: c.r, g: c.g, b: c.b }).as_bytes());
            }
            _ => {}
        }
    }
}

pub(crate) type SharedTerm = Arc<FairMutex<Term<Proxy>>>;

#[derive(Default)]
pub(crate) struct Feeder {
    scrubber: VtScrubber,
    frames: VtFrameGate,
    parser: Processor,
    buf: Vec<u8>,
    out: Vec<u8>,
    generation: u64,
}

impl Feeder {
    pub fn adopt(&mut self, generation: u64, term: &SharedTerm, port: &Port) {
        self.generation = generation;
        self.out.clear();
        self.frames.flush(&mut self.out);
        self.advance(term, port);
    }

    fn feed_from(&mut self, generation: u64, term: &SharedTerm, port: &Port, bytes: &[u8]) {
        if generation != self.generation {
            return;
        }
        self.buf.clear();
        self.scrubber.scrub_into(bytes, &mut self.buf);
        self.out.clear();
        self.frames.feed(&self.buf, Instant::now(), &mut self.out);
        self.advance(term, port);
    }

    /// The app's own lines, after whatever frame the pty left open.
    pub fn feed(&mut self, term: &SharedTerm, port: &Port, bytes: &[u8]) {
        self.out.clear();
        self.frames.flush(&mut self.out);
        self.scrubber.scrub_into(bytes, &mut self.out);
        self.advance(term, port);
    }

    fn advance(&mut self, term: &SharedTerm, port: &Port) {
        if self.out.is_empty() {
            return;
        }
        self.parser.advance(&mut *term.lock(), &self.out);
        if self.parser.sync_bytes_count() < self.out.len() {
            port.wake();
        }
        port.sticky.written();
    }

    fn deadline(&self) -> Option<Instant> {
        match (self.parser.sync_timeout().sync_timeout(), self.frames.deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn expire(&mut self, term: &SharedTerm, port: &Port) {
        let now = Instant::now();
        self.out.clear();
        self.frames.expire(now, &mut self.out);
        self.advance(term, port);
        if self.parser.sync_timeout().sync_timeout().is_some_and(|d| now >= d) {
            self.parser.stop_sync(&mut *term.lock());
            port.wake();
        }
    }
}

pub(crate) type ChildPoll<'a> = &'a mut dyn FnMut() -> Option<Option<i32>>;

pub(crate) trait LifeOwner: Send + Sync + 'static {
    fn reap(&self, generation: u64, poll: ChildPoll<'_>) -> bool;
}

pub(crate) struct LifeIo {
    pub stop: Arc<AtomicBool>,
    pub waker: UnixStream,
}

impl Drop for LifeIo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = (&self.waker).write(&[1]);
    }
}

pub(crate) fn start(
    pty: Pty,
    generation: u64,
    term: SharedTerm,
    port: Arc<Port>,
    feeder: Arc<Mutex<Feeder>>,
    owner: Weak<dyn LifeOwner>,
) -> anyhow::Result<LifeIo> {
    let (wake_tx, wake_rx) = UnixStream::pair()?;
    wake_rx.set_nonblocking(true)?;
    wake_tx.set_nonblocking(true)?;
    let writer = Writer {
        generation,
        file: pty.file().try_clone()?,
        pending: Vec::new(),
        waker: wake_tx.try_clone()?,
    };
    if let Ok(mut slot) = port.writer.lock() {
        *slot = Some(writer);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    std::thread::Builder::new()
        .name(format!("mx-pty-{generation}"))
        .spawn(move || {
            let mut life = Life { pty, generation, term, port, feeder, owner, wake_rx, stop: thread_stop };
            life.run();
        })?;
    Ok(LifeIo { stop, waker: wake_tx })
}

struct Life {
    pty: Pty,
    generation: u64,
    term: SharedTerm,
    port: Arc<Port>,
    feeder: Arc<Mutex<Feeder>>,
    owner: Weak<dyn LifeOwner>,
    wake_rx: UnixStream,
    stop: Arc<AtomicBool>,
}

impl Life {
    fn run(&mut self) {
        let mut buf = vec![0u8; READ_CHUNK];
        let mut hung_up = false;
        let mut reported = false;
        let mut next_check = Instant::now() + CHILD_CHECK;
        let mut recheck = HUP_RECHECK;
        while !self.stop.load(Ordering::Acquire) {
            let wants_write = !hung_up && self.has_pending();
            let timeout = match (hung_up, self.deadline()) {
                (true, _) => recheck,
                (false, Some(d)) => d.saturating_duration_since(Instant::now()).min(CHILD_CHECK),
                (false, None) => CHILD_CHECK,
            };
            let (pty_ready, hup) = self.wait(timeout, wants_write, hung_up);
            self.drain_waker();
            if self.stop.load(Ordering::Acquire) {
                break;
            }
            if pty_ready && !hung_up && self.read_all(&mut buf) {
                hung_up = true;
            }
            hung_up |= hup;
            if wants_write {
                self.flush();
            }
            if let Some(d) = self.deadline()
                && Instant::now() >= d
                && let Ok(mut f) = self.feeder.lock()
                && f.generation == self.generation
            {
                f.expire(&self.term, &self.port);
            }
            if !reported && (hung_up || Instant::now() >= next_check) {
                next_check = Instant::now() + CHILD_CHECK;
                let Some(owner) = self.owner.upgrade() else { break };
                let pty = &mut self.pty;
                reported = owner.reap(self.generation, &mut || match pty.next_child_event() {
                    Some(ChildEvent::Exited(status)) => Some(status.and_then(|s| exit_code(&s))),
                    _ => None,
                });
                if hung_up {
                    recheck = (recheck * 2).min(CHILD_CHECK);
                }
            }
            if hung_up && reported {
                break;
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.port.writer.lock()
            && let Some(writer) = guard.as_mut()
            && writer.generation == self.generation
        {
            writer.flush();
        }
    }

    fn has_pending(&self) -> bool {
        self.port
            .writer
            .lock()
            .map(|w| w.as_ref().is_some_and(|w| w.generation == self.generation && !w.pending.is_empty()))
            .unwrap_or(false)
    }

    fn deadline(&self) -> Option<Instant> {
        self.feeder.lock().ok().and_then(|f| f.deadline())
    }

    fn wait(&self, timeout: Duration, write: bool, hung_up: bool) -> (bool, bool) {
        let ts = Timespec {
            tv_sec: timeout.as_secs() as _,
            tv_nsec: timeout.subsec_nanos() as _,
        };
        if hung_up {
            let _ = poll(&mut [PollFd::new(&self.wake_rx, PollFlags::IN)], Some(&ts));
            return (false, false);
        }
        let mut flags = PollFlags::IN;
        if write {
            flags |= PollFlags::OUT;
        }
        let mut fds = [
            PollFd::new(self.pty.file(), flags),
            PollFd::new(&self.wake_rx, PollFlags::IN),
        ];
        if poll(&mut fds, Some(&ts)).is_err() {
            return (false, false);
        }
        let ev = fds[0].revents();
        let hup = ev.intersects(PollFlags::HUP | PollFlags::ERR) && !ev.contains(PollFlags::IN);
        (ev.contains(PollFlags::IN), hup)
    }

    fn drain_waker(&self) {
        let mut sink = [0u8; 64];
        while matches!((&self.wake_rx).read(&mut sink), Ok(n) if n > 0) {}
    }

    fn read_all(&mut self, buf: &mut [u8]) -> bool {
        let mut processed = 0;
        loop {
            match self.pty.reader().read(buf) {
                Ok(0) => return true,
                Ok(n) => {
                    if let Ok(mut f) = self.feeder.lock() {
                        f.feed_from(self.generation, &self.term, &self.port, &buf[..n]);
                    }
                    processed += n;
                    if processed >= MAX_LOCKED_READ {
                        return false;
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) if e.kind() == ErrorKind::WouldBlock => return false,
                Err(_) => return true,
            }
        }
    }
}

impl Drop for Life {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.port.writer.lock()
            && slot.as_ref().is_some_and(|w| w.generation == self.generation)
        {
            *slot = None;
        }
    }
}

pub(crate) fn exit_code(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.code().or_else(|| status.signal().map(|s| -s))
}
