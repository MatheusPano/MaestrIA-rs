use std::fs::OpenOptions;
use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use super::Stream as UnixStream;
use super::wire::{self, Attach, Decoder, Exit, Failure, Finished, Hibernate, Info, Prune, Ready, Size, Spawn};
use crate::io::{Feeder, LifeIo, LifeOwner, Port, SharedTerm, Sink, Wake, Writer, release_writer};
use crate::session::GridSize;

const HANDSHAKE: Duration = Duration::from_secs(5);
const START_WAIT: Duration = Duration::from_secs(3);
const READ_CHUNK: usize = 0x10000;

#[derive(Clone, Debug)]
pub struct Ptyd {
    socket: PathBuf,
}

impl Ptyd {
    pub fn at(socket: impl Into<PathBuf>) -> Ptyd {
        Ptyd { socket: socket.into() }
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub fn ensure(socket: impl Into<PathBuf>, exe: &Path, log: Option<&Path>) -> io::Result<(Ptyd, Info)> {
        let ptyd = Ptyd::at(socket);
        if let Ok(info) = ptyd.info() {
            return Ok((ptyd, info));
        }
        ptyd.launch(exe, log)?;
        let deadline = Instant::now() + START_WAIT;
        loop {
            match ptyd.info() {
                Ok(info) => return Ok((ptyd, info)),
                Err(err) if Instant::now() >= deadline => return Err(err),
                Err(_) => std::thread::sleep(Duration::from_millis(30)),
            }
        }
    }

    fn launch(&self, exe: &Path, log: Option<&Path>) -> io::Result<()> {
        let stderr = log
            .and_then(|p| OpenOptions::new().create(true).append(true).open(p).ok())
            .map_or_else(Stdio::null, Stdio::from);
        let exe = daemon_exe(&self.socket, exe);
        let mut command = Command::new(&exe);
        if cfg!(windows) {
            mx_core::proc::isolate(&mut command);
        }
        let mut child = command
            .arg(super::FLAG)
            .arg(&self.socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(stderr)
            .spawn()?;
        std::thread::spawn(move || child.wait());
        Ok(())
    }

    fn connect(&self) -> io::Result<UnixStream> {
        let stream = UnixStream::connect(&self.socket)?;
        stream.set_read_timeout(Some(HANDSHAKE))?;
        stream.set_write_timeout(Some(HANDSHAKE))?;
        Ok(stream)
    }

    pub fn info(&self) -> io::Result<Info> {
        let stream = self.connect()?;
        wire::write_frame(&stream, wire::INFO, b"")?;
        match wire::read_frame(&stream)? {
            (wire::INFO, payload) => wire::parse(&payload),
            _ => Err(io::Error::new(ErrorKind::InvalidData, "resposta inesperada do ptyd")),
        }
    }

    pub fn prune(&self, keep: Vec<String>) -> io::Result<usize> {
        let stream = self.connect()?;
        wire::write_json(&stream, wire::PRUNE, &Prune { keep })?;
        match wire::read_frame(&stream)? {
            (wire::PRUNE, payload) => wire::parse(&payload),
            _ => Err(io::Error::new(ErrorKind::InvalidData, "resposta inesperada do ptyd")),
        }
    }

    pub(crate) fn take_finished(&self, key: &str) -> Option<Finished> {
        let file = wire::screen_file(&self.socket, key);
        let bytes = std::fs::read(&file).ok()?;
        let _ = std::fs::remove_file(&file);
        wire::parse(&bytes).ok()
    }

    pub fn sweep_finished(&self, keep: &[String]) {
        let Ok(entries) = std::fs::read_dir(wire::screens_dir(&self.socket)) else { return };
        let kept: Vec<PathBuf> = keep.iter().map(|k| wire::screen_file(&self.socket, k)).collect();
        for entry in entries.flatten() {
            let path = entry.path();
            if !kept.contains(&path) {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    pub fn hibernate_when_idle(&self, keys: Vec<String>, shells: Vec<String>, after: Duration) -> io::Result<()> {
        let stream = self.connect()?;
        wire::write_json(&stream, wire::HIBERNATE, &Hibernate { after_secs: after.as_secs(), keys, shells })?;
        wire::read_frame(&stream).map(drop)
    }

    pub fn shutdown(&self) -> io::Result<()> {
        let stream = self.connect()?;
        wire::write_frame(&stream, wire::SHUTDOWN, b"")?;
        let _ = wire::read_frame(&stream);
        Ok(())
    }

    pub(crate) fn spawn(&self, req: &Spawn) -> io::Result<(UnixStream, Ready)> {
        let stream = self.connect()?;
        wire::write_json(&stream, wire::SPAWN, req)?;
        let ready = reply(&stream)?;
        Ok((stream, ready))
    }

    pub(crate) fn attach(&self, key: &str) -> io::Result<(UnixStream, Ready)> {
        let stream = self.connect()?;
        wire::write_json(&stream, wire::ATTACH, &Attach { v: wire::VERSION, key: key.to_owned() })?;
        let ready = reply(&stream)?;
        Ok((stream, ready))
    }
}

#[cfg(unix)]
fn daemon_exe(_: &Path, exe: &Path) -> PathBuf {
    exe.to_path_buf()
}

// The daemon outlives the app; running it from a copy leaves the installed exe free for the MSI to replace.
#[cfg(windows)]
fn daemon_exe(socket: &Path, exe: &Path) -> PathBuf {
    let Some(stamp) = exe_stamp(exe) else { return exe.to_path_buf() };
    let copy = socket.with_file_name(format!("ptyd-{}-{stamp}.exe", super::rev()));
    if copy.is_file() {
        return copy;
    }
    let temp = copy.with_extension(format!("{}.tmp", std::process::id()));
    let copied = std::fs::copy(exe, &temp).and_then(|_| std::fs::rename(&temp, &copy));
    let _ = std::fs::remove_file(&temp);
    if copied.is_err() && !copy.is_file() {
        return exe.to_path_buf();
    }
    remove_old_copies(&copy);
    copy
}

#[cfg(windows)]
fn exe_stamp(exe: &Path) -> Option<String> {
    let meta = std::fs::metadata(exe).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(format!("{:x}{modified:x}", meta.len()))
}

#[cfg(windows)]
fn remove_old_copies(current: &Path) {
    let (Some(dir), Some(name)) = (current.parent(), current.file_name()) else { return };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let other = entry.file_name();
        let other = other.to_string_lossy();
        if other != name.to_string_lossy() && other.starts_with("ptyd-") && other.ends_with(".exe") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn reply(stream: &UnixStream) -> io::Result<Ready> {
    match wire::read_frame(stream)? {
        (wire::READY, payload) => wire::parse(&payload),
        (wire::ERROR, payload) => {
            let failure: Failure = wire::parse(&payload)?;
            Err(io::Error::other(failure.message))
        }
        _ => Err(io::Error::new(ErrorKind::InvalidData, "resposta inesperada do ptyd")),
    }
}

pub(crate) fn input_frame(bytes: &[u8]) -> Vec<u8> {
    wire::encode(wire::INPUT, bytes)
}

pub(crate) fn resize_frame(size: GridSize) -> Vec<u8> {
    wire::encode(wire::RESIZE, &serde_json::to_vec(&Size::from(size)).unwrap_or_default())
}

fn write_frames(mut stream: UnixStream, frames: std::sync::mpsc::Receiver<Vec<u8>>) {
    use std::io::Write;
    for frame in frames {
        if stream.write_all(&frame).is_err() {
            return;
        }
    }
}

pub(crate) fn start(
    stream: UnixStream,
    generation: u64,
    term: SharedTerm,
    port: Arc<Port>,
    feeder: Arc<Mutex<Feeder>>,
    owner: Weak<dyn LifeOwner>,
) -> io::Result<LifeIo> {
    let input = stream.try_clone()?;
    input.set_write_timeout(None)?;
    let (frames, queue) = std::sync::mpsc::channel();
    std::thread::Builder::new().name(format!("mx-ptyd-in-{generation}")).spawn(move || write_frames(input, queue))?;
    if let Ok(mut slot) = port.writer.lock() {
        *slot = Some(Writer { generation, sink: Sink::Remote(frames) });
    }
    let stop = Arc::new(AtomicBool::new(false));
    let wake = stream.try_clone()?;
    let thread_stop = stop.clone();
    std::thread::Builder::new().name(format!("mx-ptyd-{generation}")).spawn(move || {
        Remote { stream, generation, term, port, feeder, owner, stop: thread_stop }.run();
    })?;
    Ok(LifeIo { stop, wake: Wake::Socket(wake) })
}

struct Remote {
    stream: UnixStream,
    generation: u64,
    term: SharedTerm,
    port: Arc<Port>,
    feeder: Arc<Mutex<Feeder>>,
    owner: Weak<dyn LifeOwner>,
    stop: Arc<AtomicBool>,
}

impl Remote {
    fn run(&mut self) {
        let mut decoder = Decoder::default();
        let mut buf = vec![0u8; READ_CHUNK];
        let code = 'outer: loop {
            if self.stop.load(Ordering::Acquire) {
                return;
            }
            let timeout = self
                .deadline()
                .map(|d| d.saturating_duration_since(Instant::now()).max(Duration::from_millis(1)));
            if self.stream.set_read_timeout(timeout).is_err() {
                break None;
            }
            match self.stream.read(&mut buf) {
                Ok(0) => break None,
                Ok(n) => {
                    decoder.push(&buf[..n]);
                    while let Some(frame) = decoder.next() {
                        match frame {
                            Ok((wire::DATA, bytes)) => self.feed(&bytes),
                            Ok((wire::EXIT, payload)) => {
                                break 'outer wire::parse::<Exit>(&payload).ok().and_then(|e| e.code);
                            }
                            Ok((wire::TAKEN, _)) => {
                                self.feed(crate::spawn::remark_line("sessão aberta em outra janela").as_bytes());
                                return;
                            }
                            Ok(_) => {}
                            Err(_) => break 'outer None,
                        }
                    }
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {}
                Err(_) => break None,
            }
            self.expire();
        };
        if self.stop.load(Ordering::Acquire) {
            return;
        }
        if let Some(owner) = self.owner.upgrade() {
            owner.reap(self.generation, &mut || Some(code));
        }
    }

    fn feed(&self, bytes: &[u8]) {
        if let Ok(mut f) = self.feeder.lock() {
            f.feed_from(self.generation, &self.term, &self.port, bytes);
        }
    }

    fn deadline(&self) -> Option<Instant> {
        self.feeder.lock().ok().and_then(|f| f.deadline())
    }

    fn expire(&self) {
        if self.deadline().is_some_and(|d| Instant::now() >= d)
            && let Ok(mut f) = self.feeder.lock()
            && f.generation == self.generation
        {
            f.expire(&self.term, &self.port);
        }
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        release_writer(&self.port, self.generation);
    }
}

pub(crate) fn spawn_request(
    key: &str,
    plan: crate::spawn::SpawnPlan,
    scrollback: usize,
    size: GridSize,
) -> Spawn {
    Spawn {
        v: wire::VERSION,
        key: key.to_owned(),
        program: plan.program,
        args: plan.args,
        cwd: plan.cwd,
        env: plan.env,
        scrollback,
        size: size.into(),
    }
}
