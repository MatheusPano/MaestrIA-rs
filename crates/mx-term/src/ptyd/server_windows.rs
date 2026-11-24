use std::io::{self, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use uds_windows::UnixListener;

use super::{ACCEPT_TICK, Daemon, Session, UnixStream, finish};
use crate::ptyd::wire::Spawn;
use crate::session::GridSize;
use crate::signals;

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

    pub(super) fn redraw(&self) {}

    pub(super) fn input(&self, bytes: &[u8]) {
        if self.alive() {
            self.pty.write(bytes);
        }
    }

    pub(super) fn resize_pty(&self, size: GridSize) {
        self.pty.resize(size);
    }
}
