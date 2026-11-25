use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alacritty_terminal::tty::{self, ChildEvent, EventedPty, EventedReadWrite, Pty};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use super::{ACCEPT_TICK, Daemon, Session, UnixStream, finish, lock};
use crate::ptyd::wire::Spawn;
use crate::session::GridSize;
use crate::signals;

pub(super) const READ_CHUNK: usize = 0x10000;
pub(super) const CHILD_CHECK: Duration = Duration::from_millis(500);
pub(super) const HUP_RECHECK: Duration = Duration::from_millis(10);
pub(super) const INPUT_RECHECK: Duration = Duration::from_secs(1);

pub(super) fn window(size: GridSize) -> alacritty_terminal::event::WindowSize {
    alacritty_terminal::event::WindowSize {
        num_lines: size.rows,
        num_cols: size.cols,
        cell_width: size.cell_width,
        cell_height: size.cell_height,
    }
}

pub(super) fn timespec(d: Duration) -> Timespec {
    Timespec { tv_sec: d.as_secs() as _, tv_nsec: d.subsec_nanos() as _ }
}

pub(super) type Clients = UnixListener;

pub(super) fn clients(listener: UnixListener) -> io::Result<Clients> {
    listener.set_nonblocking(true)?;
    Ok(listener)
}

pub(super) fn accept(listener: &Clients) -> io::Result<UnixStream> {
    listener.accept().map(|(stream, _)| stream)
}

pub(super) fn wait_for_client(listener: &Clients) {
    let _ = poll(&mut [PollFd::new(listener, PollFlags::IN)], Some(&timespec(ACCEPT_TICK)));
}

pub(super) fn open_pty(req: &Spawn, size: GridSize) -> anyhow::Result<(Pty, u32, Mutex<File>)> {
    let options = tty::Options {
        shell: Some(tty::Shell::new(req.program.clone(), req.args.clone())),
        working_directory: req.cwd.clone(),
        drain_on_exit: true,
        env: req.env.clone(),
    };
    let pty = tty::new(&options, window(size), 0)?;
    let pid = pty.child().id();
    let file = pty.file().try_clone()?;
    Ok((pty, pid, Mutex::new(file)))
}

pub(super) fn pump(daemon: &Arc<Daemon>, session: &Arc<Session>, mut pty: Pty) {
    let mut buf = vec![0u8; READ_CHUNK];
    let mut hung_up = false;
    let mut next_check = Instant::now() + CHILD_CHECK;
    let mut recheck = HUP_RECHECK;
    let code = loop {
        let timeout = if hung_up {
            recheck
        } else {
            session
                .sync_deadline()
                .map_or(CHILD_CHECK, |d| d.saturating_duration_since(Instant::now()).min(CHILD_CHECK))
        };
        if hung_up {
            std::thread::sleep(timeout);
        } else {
            let mut fds = [PollFd::new(pty.file(), PollFlags::IN)];
            if poll(&mut fds, Some(&timespec(timeout))).is_ok() {
                let ev = fds[0].revents();
                if ev.contains(PollFlags::IN) {
                    hung_up = read_all(session, &mut pty, &mut buf);
                } else if ev.intersects(PollFlags::HUP | PollFlags::ERR) {
                    hung_up = true;
                }
            }
        }
        session.expire_sync();
        if hung_up || Instant::now() >= next_check {
            next_check = Instant::now() + CHILD_CHECK;
            if session.due_to_hibernate() {
                log::info!("ptyd hibernating idle session {}", session.key);
                signals::hang_up(session.pid);
            }
            if let Some(ChildEvent::Exited(status)) = pty.next_child_event() {
                break status.and_then(|s| signals::exit_code(&s));
            }
            if hung_up {
                recheck = (recheck * 2).min(CHILD_CHECK);
            }
        }
    };
    finish(daemon, session, code);
}

pub(super) fn read_all(session: &Session, pty: &mut Pty, buf: &mut [u8]) -> bool {
    loop {
        match pty.reader().read(buf) {
            Ok(0) => return true,
            Ok(n) => session.output(&buf[..n]),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => return false,
            Err(_) => return true,
        }
    }
}

impl Session {
    pub(super) fn idle_shell(&self) -> bool {
        signals::idle_shell(&*lock(&self.pty), self.pid)
    }

    pub(super) fn redraw(&self) {
        signals::redraw(&*lock(&self.pty));
    }

    pub(super) fn input(&self, mut bytes: &[u8]) {
        let file = lock(&self.pty);
        while !bytes.is_empty() && self.alive() {
            match (&*file).write(bytes) {
                Ok(0) => return,
                Ok(n) => bytes = &bytes[n..],
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    let _ = poll(&mut [PollFd::new(&*file, PollFlags::OUT)], Some(&timespec(INPUT_RECHECK)));
                }
                Err(_) => return,
            }
        }
    }

    pub(super) fn resize_pty(&self, size: GridSize) {
        let file = lock(&self.pty);
        let ws = rustix::termios::Winsize {
            ws_row: size.rows,
            ws_col: size.cols,
            ws_xpixel: size.cols.saturating_mul(size.cell_width),
            ws_ypixel: size.rows.saturating_mul(size.cell_height),
        };
        let _ = rustix::termios::tcsetwinsize(file.as_fd(), ws);
    }

    pub(super) fn after_detach(&self) {}
}
