use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};

use crate::session::GridSize;

const READ_CHUNK: usize = 0x10000;
const MAX_BATCH: usize = 0x40000;
const CHILD_CHECK: Duration = Duration::from_millis(500);
const DRAIN_AFTER_EXIT: Duration = Duration::from_secs(2);

pub(crate) enum Msg {
    Data(Vec<u8>),
    Eof,
    Wake,
}

type Master = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;

#[derive(Clone)]
pub(crate) struct Handle {
    input: Sender<Vec<u8>>,
    master: Master,
}

impl Handle {
    pub fn write(&self, bytes: &[u8]) {
        if !bytes.is_empty() {
            let _ = self.input.send(bytes.to_vec());
        }
    }

    pub fn resize(&self, size: GridSize) {
        if let Ok(guard) = self.master.lock()
            && let Some(master) = guard.as_ref()
        {
            let _ = master.resize(pty_size(size));
        }
    }

    fn close(&self) {
        if let Ok(mut guard) = self.master.lock() {
            guard.take();
        }
    }
}

pub(crate) struct Conpty {
    pub pid: u32,
    pub handle: Handle,
    child: Box<dyn Child + Send + Sync>,
    output: Receiver<Msg>,
    wake: Sender<Msg>,
}

fn pty_size(size: GridSize) -> PtySize {
    PtySize {
        rows: size.rows.max(1),
        cols: size.cols.max(1),
        pixel_width: size.cols.saturating_mul(size.cell_width),
        pixel_height: size.rows.saturating_mul(size.cell_height),
    }
}

pub(crate) fn spawn(
    program: &str,
    args: &[String],
    cwd: Option<&PathBuf>,
    env: &HashMap<String, String>,
    size: GridSize,
) -> anyhow::Result<Conpty> {
    let pair = portable_pty::native_pty_system().openpty(pty_size(size))?;
    let mut cmd = CommandBuilder::new(program);
    cmd.args(args);
    cmd.env_clear();
    for (k, v) in env {
        cmd.env(k, v);
    }
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }
    let child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);
    let pid = child.process_id().unwrap_or_default();
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    let (out_tx, output) = std::sync::mpsc::channel();
    let wake = out_tx.clone();
    std::thread::Builder::new().name(format!("mx-conpty-out-{pid}")).spawn(move || {
        let mut buf = vec![0u8; READ_CHUNK];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if out_tx.send(Msg::Data(buf[..n].to_vec())).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = out_tx.send(Msg::Eof);
    })?;
    let (input, in_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::Builder::new().name(format!("mx-conpty-in-{pid}")).spawn(move || {
        for bytes in in_rx {
            if writer.write_all(&bytes).and_then(|()| writer.flush()).is_err() {
                return;
            }
        }
    })?;
    let handle = Handle { input, master: Arc::new(Mutex::new(Some(pair.master))) };
    Ok(Conpty { pid, handle, child, output, wake })
}

pub(crate) struct Hooks<'a> {
    pub stop: &'a AtomicBool,
    pub deadline: &'a mut dyn FnMut() -> Option<Instant>,
    pub data: &'a mut dyn FnMut(&[u8]),
    pub tick: &'a mut dyn FnMut(),
}

impl Conpty {
    pub fn waker(&self) -> Sender<Msg> {
        self.wake.clone()
    }

    fn exit_code(&mut self) -> Option<Option<i32>> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(Some(status.exit_code() as i32)),
            Ok(None) => None,
            Err(_) => Some(None),
        }
    }

    pub fn run(&mut self, hooks: Hooks<'_>) -> Option<Option<i32>> {
        let mut exited: Option<(Option<i32>, Instant)> = None;
        let mut next_check = Instant::now();
        loop {
            if hooks.stop.load(Ordering::Acquire) {
                return None;
            }
            if exited.is_none() && Instant::now() >= next_check {
                next_check = Instant::now() + CHILD_CHECK;
                if let Some(code) = self.exit_code() {
                    exited = Some((code, Instant::now()));
                    self.handle.close();
                }
            }
            if let Some((code, at)) = exited
                && at.elapsed() >= DRAIN_AFTER_EXIT
            {
                return Some(code);
            }
            let limit = exited.map_or(next_check, |(_, at)| at + DRAIN_AFTER_EXIT);
            let wait = (hooks.deadline)().map_or(limit, |d| d.min(limit)).saturating_duration_since(Instant::now());
            match self.output.recv_timeout(wait) {
                Ok(Msg::Data(bytes)) => {
                    (hooks.data)(&bytes);
                    let mut batched = bytes.len();
                    while batched < MAX_BATCH {
                        match self.output.try_recv() {
                            Ok(Msg::Data(more)) => {
                                (hooks.data)(&more);
                                batched += more.len();
                            }
                            Ok(Msg::Eof) => return Some(self.wait_exit(exited)),
                            _ => break,
                        }
                    }
                }
                Ok(Msg::Eof) | Err(RecvTimeoutError::Disconnected) => return Some(self.wait_exit(exited)),
                Ok(Msg::Wake) | Err(RecvTimeoutError::Timeout) => {}
            }
            (hooks.tick)();
        }
    }

    fn wait_exit(&mut self, exited: Option<(Option<i32>, Instant)>) -> Option<i32> {
        if let Some((code, _)) = exited {
            return code;
        }
        self.handle.close();
        match self.child.wait() {
            Ok(status) => Some(status.exit_code() as i32),
            Err(_) => None,
        }
    }
}

impl Drop for Conpty {
    fn drop(&mut self) {
        self.handle.close();
    }
}
