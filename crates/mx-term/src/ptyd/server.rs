use std::collections::HashMap;
#[cfg(unix)]
use std::fs::File;
use std::io::{self, ErrorKind};
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::fd::AsFd;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
#[cfg(windows)]
use uds_windows::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::{Config, Term};
#[cfg(unix)]
use alacritty_terminal::tty::{self, ChildEvent, EventedPty, EventedReadWrite, Pty};
use alacritty_terminal::vte::ansi::Processor;
#[cfg(unix)]
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use super::Stream as UnixStream;
use super::replay::replay;
use super::wire::{self, Attach, Exit, Failure, Finished, Hibernate, Info, Prune, Ready, Size, Spawn};
use crate::scrubber::VtScrubber;
use crate::session::GridSize;
use crate::signals;

#[cfg(unix)]
const READ_CHUNK: usize = 0x10000;
#[cfg(unix)]
const CHILD_CHECK: Duration = Duration::from_millis(500);
#[cfg(unix)]
const HUP_RECHECK: Duration = Duration::from_millis(10);
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(unix)]
const INPUT_RECHECK: Duration = Duration::from_secs(1);
#[cfg(unix)]
const ACCEPT_TICK: Duration = Duration::from_secs(1);
#[cfg(windows)]
const ACCEPT_TICK: Duration = Duration::from_secs(1);
const IDLE_EXIT: Duration = Duration::from_secs(60);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(unix)]
fn window(size: GridSize) -> alacritty_terminal::event::WindowSize {
    alacritty_terminal::event::WindowSize {
        num_lines: size.rows,
        num_cols: size.cols,
        cell_width: size.cell_width,
        cell_height: size.cell_height,
    }
}

#[cfg(unix)]
fn timespec(d: Duration) -> Timespec {
    Timespec { tv_sec: d.as_secs() as _, tv_nsec: d.subsec_nanos() as _ }
}

struct Client {
    id: u64,
    stream: UnixStream,
}

struct Mirror {
    term: Term<VoidListener>,
    parser: Processor,
    scrubber: VtScrubber,
    scrubbed: Vec<u8>,
    size: GridSize,
    client: Option<Client>,
    exited: Option<Option<i32>>,
    quiet_since: Instant,
    hibernate_after: Option<Duration>,
    hibernating: bool,
    idle_shell: bool,
}

impl Mirror {
    fn send(&mut self, kind: u8, payload: &[u8]) {
        let Some(client) = self.client.as_mut() else { return };
        if wire::write_frame(&client.stream, kind, payload).is_err() {
            let _ = client.stream.shutdown(std::net::Shutdown::Both);
            self.client = None;
        }
    }

    fn send_json(&mut self, kind: u8, value: &impl serde::Serialize) {
        if let Ok(bytes) = serde_json::to_vec(value) {
            self.send(kind, &bytes);
        }
    }

    fn take(&mut self, client: Client) {
        if let Some(old) = self.client.replace(client) {
            let _ = wire::write_frame(&old.stream, wire::TAKEN, b"");
            let _ = old.stream.shutdown(std::net::Shutdown::Both);
        }
        self.hibernate_after = None;
        self.hibernating = false;
        self.idle_shell = false;
    }

    fn ready(&self, pid: u32) -> Ready {
        Ready {
            pid: self.exited.is_none().then_some(pid),
            exited: self.exited.is_some(),
            code: self.exited.flatten(),
            size: self.size.into(),
        }
    }
}

struct Session {
    key: String,
    pid: u32,
    done: AtomicBool,
    #[cfg(unix)]
    pty: Mutex<File>,
    #[cfg(windows)]
    pty: crate::conpty::Handle,
    mirror: Mutex<Mirror>,
}

impl Session {
    fn alive(&self) -> bool {
        !self.done.load(Ordering::Acquire)
    }

    fn output(&self, bytes: &[u8]) {
        let mut m = lock(&self.mirror);
        let m = &mut *m;
        m.scrubbed.clear();
        m.scrubber.scrub_into(bytes, &mut m.scrubbed);
        m.parser.advance(&mut m.term, &m.scrubbed);
        m.quiet_since = Instant::now();
        m.send(wire::DATA, bytes);
    }

    fn due_to_hibernate(&self) -> bool {
        let mut m = lock(&self.mirror);
        let due = m.client.is_none()
            && !m.hibernating
            && m.exited.is_none()
            && m.hibernate_after.is_some_and(|after| m.quiet_since.elapsed() >= after)
            && (!m.idle_shell || self.idle_shell());
        if due {
            m.hibernating = true;
        }
        due
    }

    fn sync_deadline(&self) -> Option<Instant> {
        lock(&self.mirror).parser.sync_timeout().sync_timeout()
    }

    fn expire_sync(&self) {
        let mut m = lock(&self.mirror);
        let m = &mut *m;
        if m.parser.sync_timeout().sync_timeout().is_some_and(|d| Instant::now() >= d) {
            m.parser.stop_sync(&mut m.term);
        }
    }

    #[cfg(unix)]
    fn idle_shell(&self) -> bool {
        signals::idle_shell(&*lock(&self.pty), self.pid)
    }

    #[cfg(windows)]
    fn idle_shell(&self) -> bool {
        false
    }

    #[cfg(unix)]
    fn redraw(&self) {
        signals::redraw(&*lock(&self.pty));
    }

    #[cfg(windows)]
    fn redraw(&self) {}

    #[cfg(windows)]
    fn input(&self, bytes: &[u8]) {
        if self.alive() {
            self.pty.write(bytes);
        }
    }

    #[cfg(unix)]
    fn input(&self, mut bytes: &[u8]) {
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

    fn resize(&self, size: GridSize) {
        let mut m = lock(&self.mirror);
        if m.size == size {
            return;
        }
        m.size = size;
        m.term.resize(size);
        drop(m);
        self.resize_pty(size);
    }

    #[cfg(windows)]
    fn resize_pty(&self, size: GridSize) {
        self.pty.resize(size);
    }

    #[cfg(unix)]
    fn resize_pty(&self, size: GridSize) {
        let file = lock(&self.pty);
        let ws = rustix::termios::Winsize {
            ws_row: size.rows,
            ws_col: size.cols,
            ws_xpixel: size.cols.saturating_mul(size.cell_width),
            ws_ypixel: size.rows.saturating_mul(size.cell_height),
        };
        let _ = rustix::termios::tcsetwinsize(file.as_fd(), ws);
    }

    fn detach(&self, client: u64) {
        let mut m = lock(&self.mirror);
        if m.client.as_ref().is_some_and(|c| c.id == client) {
            m.client = None;
            m.quiet_since = Instant::now();
        }
    }
}

#[derive(Default)]
struct Daemon {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    conns: AtomicUsize,
    next_client: AtomicU64,
    rev: String,
    socket: PathBuf,
}

impl Daemon {
    fn idle(&self) -> bool {
        self.conns.load(Ordering::Acquire) == 0 && lock(&self.sessions).is_empty()
    }

    fn forget(&self, session: &Arc<Session>) {
        let mut map = lock(&self.sessions);
        if map.get(&session.key).is_some_and(|s| Arc::ptr_eq(s, session)) {
            map.remove(&session.key);
        }
    }

    fn prune(&self, keep: &[String]) -> usize {
        let mut map = lock(&self.sessions);
        map.retain(|key, s| keep.contains(key) || s.alive());
        let orphans: Vec<Arc<Session>> = map.values().filter(|s| !keep.contains(&s.key)).cloned().collect();
        drop(map);
        for s in &orphans {
            signals::hang_up(s.pid);
        }
        let ended = orphans.len();
        if ended > 0 {
            std::thread::spawn(move || {
                std::thread::sleep(signals::GRACE);
                for s in orphans.iter().filter(|s| s.alive()) {
                    signals::force_kill(s.pid);
                }
            });
        }
        ended
    }

    fn hang_up_all(&self) {
        for s in lock(&self.sessions).values().filter(|s| s.alive()) {
            signals::hang_up(s.pid);
        }
    }
}

pub fn run(socket: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    let _ = rustix::process::setsid();
    serve(socket)
}

pub fn serve(socket: &Path) -> anyhow::Result<()> {
    let rev = super::rev();
    let listener = clients(bind(socket)?)?;
    let daemon = Arc::new(Daemon { rev: rev.clone(), socket: socket.to_owned(), ..Daemon::default() });
    let stop = Arc::new(AtomicBool::new(false));
    let mut idle_since: Option<Instant> = None;
    log::info!("ptyd {rev} listening on {}", socket.display());
    while !stop.load(Ordering::Acquire) {
        match accept(&listener) {
            Ok(stream) => {
                if !same_user(&stream) {
                    continue;
                }
                let _ = stream.set_nonblocking(false);
                daemon.conns.fetch_add(1, Ordering::AcqRel);
                let (d, stop) = (daemon.clone(), stop.clone());
                let spawned = std::thread::Builder::new().name("ptyd-conn".into()).spawn(move || {
                    if let Err(err) = handle(&d, stream, &stop) {
                        log::debug!("ptyd connection ended: {err}");
                    }
                    d.conns.fetch_sub(1, Ordering::AcqRel);
                });
                if spawned.is_err() {
                    daemon.conns.fetch_sub(1, Ordering::AcqRel);
                }
                continue;
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => wait_for_client(&listener),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
        if daemon.idle() {
            if idle_since.get_or_insert_with(Instant::now).elapsed() >= IDLE_EXIT {
                break;
            }
        } else {
            idle_since = None;
        }
    }
    if !stop.load(Ordering::Acquire) {
        let _ = std::fs::remove_file(socket);
    }
    log::info!("ptyd {rev} leaving");
    Ok(())
}

#[cfg(unix)]
type Clients = UnixListener;

#[cfg(unix)]
fn clients(listener: UnixListener) -> io::Result<Clients> {
    listener.set_nonblocking(true)?;
    Ok(listener)
}

#[cfg(unix)]
fn accept(listener: &Clients) -> io::Result<UnixStream> {
    listener.accept().map(|(stream, _)| stream)
}

#[cfg(unix)]
fn wait_for_client(listener: &Clients) {
    let _ = poll(&mut [PollFd::new(listener, PollFlags::IN)], Some(&timespec(ACCEPT_TICK)));
}

#[cfg(windows)]
type Clients = std::sync::mpsc::Receiver<io::Result<UnixStream>>;

#[cfg(windows)]
fn clients(listener: UnixListener) -> io::Result<Clients> {
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

#[cfg(windows)]
fn accept(clients: &Clients) -> io::Result<UnixStream> {
    use std::sync::mpsc::RecvTimeoutError;
    match clients.recv_timeout(ACCEPT_TICK) {
        Ok(accepted) => accepted,
        Err(RecvTimeoutError::Timeout) => Err(ErrorKind::WouldBlock.into()),
        Err(RecvTimeoutError::Disconnected) => Err(io::Error::other("ptyd accept thread ended")),
    }
}

#[cfg(windows)]
fn wait_for_client(_: &Clients) {}

fn bind(socket: &Path) -> io::Result<UnixListener> {
    if let Some(dir) = socket.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let listener = match UnixListener::bind(socket) {
        Ok(l) => l,
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            if UnixStream::connect(socket).is_ok() {
                return Err(io::Error::new(ErrorKind::AddrInUse, "another ptyd is already listening"));
            }
            std::fs::remove_file(socket)?;
            UnixListener::bind(socket)?
        }
        Err(e) => return Err(e),
    };
    #[cfg(unix)]
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

#[cfg(target_os = "linux")]
fn same_user(stream: &UnixStream) -> bool {
    rustix::net::sockopt::socket_peercred(stream).is_ok_and(|c| c.uid == rustix::process::getuid())
}

// macOS has no SO_PEERCRED and Windows no peer uid; there the private socket file keeps other users out.
#[cfg(not(target_os = "linux"))]
fn same_user(_: &UnixStream) -> bool {
    true
}

fn handle(daemon: &Arc<Daemon>, stream: UnixStream, stop: &AtomicBool) -> io::Result<()> {
    stream.set_write_timeout(Some(CLIENT_WRITE_TIMEOUT))?;
    let mut reader = stream.try_clone()?;
    let (kind, payload) = wire::read_frame(&mut reader)?;
    let id = daemon.next_client.fetch_add(1, Ordering::AcqRel);
    let session = match kind {
        wire::INFO => {
            let sessions = lock(&daemon.sessions).values().filter(|s| s.alive()).count();
            let info = Info { v: wire::VERSION, rev: daemon.rev.clone(), pid: std::process::id(), sessions };
            return wire::write_json(&stream, wire::INFO, &info);
        }
        wire::HIBERNATE => {
            let req: Hibernate = wire::parse(&payload)?;
            let after = (req.after_secs > 0).then(|| Duration::from_secs(req.after_secs));
            let wanted = req.keys.iter().map(|k| (k, false)).chain(req.shells.iter().map(|k| (k, true)));
            for (key, shell) in wanted {
                if let Some(s) = lock(&daemon.sessions).get(key).cloned() {
                    let mut m = lock(&s.mirror);
                    m.hibernate_after = after;
                    m.idle_shell = shell;
                }
            }
            return wire::write_frame(&stream, wire::HIBERNATE, b"");
        }
        wire::PRUNE => {
            let req: Prune = wire::parse(&payload)?;
            let ended = daemon.prune(&req.keep);
            return wire::write_json(&stream, wire::PRUNE, &ended);
        }
        wire::SHUTDOWN => {
            let _ = std::fs::remove_file(&daemon.socket);
            daemon.hang_up_all();
            stop.store(true, Ordering::Release);
            return wire::write_frame(&stream, wire::SHUTDOWN, b"");
        }
        wire::SPAWN => {
            let req: Spawn = wire::parse(&payload)?;
            if req.v != wire::VERSION {
                return refuse(&stream, &format!("ptyd fala o protocolo {}, não {}", wire::VERSION, req.v));
            }
            match spawn(daemon, req, Client { id, stream: stream.try_clone()? }) {
                Ok(s) => s,
                Err(err) => return refuse(&stream, &format!("{err:#}")),
            }
        }
        wire::ATTACH => {
            let req: Attach = wire::parse(&payload)?;
            if req.v != wire::VERSION {
                return refuse(&stream, &format!("ptyd fala o protocolo {}, não {}", wire::VERSION, req.v));
            }
            match attach(daemon, &req.key, Client { id, stream: stream.try_clone()? }) {
                Some(s) => s,
                None => return Ok(()),
            }
        }
        _ => return refuse(&stream, "primeira mensagem inesperada"),
    };
    loop {
        match wire::read_frame(&mut reader) {
            Ok((wire::INPUT, bytes)) => session.input(&bytes),
            Ok((wire::RESIZE, p)) => {
                if let Ok(size) = wire::parse::<Size>(&p) {
                    session.resize(size.into());
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    session.detach(id);
    Ok(())
}

fn refuse(stream: &UnixStream, message: &str) -> io::Result<()> {
    wire::write_json(stream, wire::ERROR, &Failure { message: message.to_owned() })
}

fn spawn(daemon: &Arc<Daemon>, req: Spawn, client: Client) -> anyhow::Result<Arc<Session>> {
    if lock(&daemon.sessions).get(&req.key).is_some_and(|old| old.alive()) {
        anyhow::bail!("a sessão {} já está viva", req.key);
    }
    let size = GridSize::from(req.size);
    let (pty, pid, handle) = open_pty(&req, size)?;
    let config = Config { scrolling_history: req.scrollback, ..Config::default() };
    let mirror = Mirror {
        term: Term::new(config, &size, VoidListener),
        parser: Processor::new(),
        scrubber: VtScrubber::new(),
        scrubbed: Vec::new(),
        size,
        client: Some(client),
        exited: None,
        quiet_since: Instant::now(),
        hibernate_after: None,
        hibernating: false,
        idle_shell: false,
    };
    let session = Arc::new(Session {
        key: req.key.clone(),
        pid,
        done: AtomicBool::new(false),
        pty: handle,
        mirror: Mutex::new(mirror),
    });
    lock(&daemon.sessions).insert(req.key, session.clone());
    let mut guard = lock(&session.mirror);
    let (d, s) = (daemon.clone(), session.clone());
    if let Err(err) = std::thread::Builder::new().name(format!("ptyd-pty-{pid}")).spawn(move || pump(&d, &s, pty)) {
        drop(guard);
        daemon.forget(&session);
        signals::hang_up(pid);
        return Err(err.into());
    }
    let ready = guard.ready(pid);
    guard.send_json(wire::READY, &ready);
    drop(guard);
    Ok(session)
}

fn attach(daemon: &Arc<Daemon>, key: &str, client: Client) -> Option<Arc<Session>> {
    let Some(session) = lock(&daemon.sessions).get(key).cloned() else {
        let _ = refuse(&client.stream, &format!("a sessão {key} não existe"));
        return None;
    };
    let mut guard = lock(&session.mirror);
    let m = &mut *guard;
    if m.exited.is_some() {
        let ready = m.ready(session.pid);
        drop(guard);
        let _ = wire::write_json(&client.stream, wire::READY, &ready);
        daemon.forget(&session);
        return None;
    }
    if m.parser.sync_bytes_count() > 0 {
        m.parser.stop_sync(&mut m.term);
    }
    let screen = replay(&m.term);
    m.take(client);
    let ready = m.ready(session.pid);
    m.send_json(wire::READY, &ready);
    m.send(wire::DATA, &screen);
    drop(guard);
    session.redraw();
    Some(session)
}

#[cfg(unix)]
fn open_pty(req: &Spawn, size: GridSize) -> anyhow::Result<(Pty, u32, Mutex<File>)> {
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

#[cfg(windows)]
fn open_pty(req: &Spawn, size: GridSize) -> anyhow::Result<(crate::conpty::Conpty, u32, crate::conpty::Handle)> {
    let pty = crate::conpty::spawn(&req.program, &req.args, req.cwd.as_ref(), &req.env, size)?;
    let (pid, handle) = (pty.pid, pty.handle.clone());
    Ok((pty, pid, handle))
}

#[cfg(windows)]
fn pump(daemon: &Arc<Daemon>, session: &Arc<Session>, mut pty: crate::conpty::Conpty) {
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

#[cfg(unix)]
fn pump(daemon: &Arc<Daemon>, session: &Arc<Session>, mut pty: Pty) {
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

fn finish(daemon: &Arc<Daemon>, session: &Arc<Session>, code: Option<i32>) {
    let mut m = lock(&session.mirror);
    m.exited = Some(code);
    session.done.store(true, Ordering::Release);
    m.send_json(wire::EXIT, &Exit { code });
    match m.client.take() {
        Some(client) => {
            let _ = client.stream.shutdown(std::net::Shutdown::Both);
        }
        None => {
            if let Err(err) = save_finished(&daemon.socket, &session.key, &m, code) {
                log::warn!("ptyd could not save the screen of {}: {err}", session.key);
            }
        }
    }
    drop(m);
    daemon.forget(session);
}

fn save_finished(socket: &Path, key: &str, m: &Mirror, code: Option<i32>) -> io::Result<()> {
    let file = wire::screen_file(socket, key);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let screen = String::from_utf8_lossy(&replay(&m.term)).into_owned();
    let finished = Finished { code, size: m.size.into(), screen, hibernated: m.hibernating };
    let temp = file.with_extension("tmp");
    std::fs::write(&temp, serde_json::to_vec(&finished).map_err(io::Error::other)?)?;
    #[cfg(unix)]
    std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))?;
    std::fs::rename(&temp, &file)
}

#[cfg(unix)]
fn read_all(session: &Session, pty: &mut Pty, buf: &mut [u8]) -> bool {
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

pub fn socket_in(state_dir: &Path) -> PathBuf {
    state_dir.join("ptyd.sock")
}
