use std::collections::HashMap;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mx_core::json::Obj;
use mx_core::ports::BoxFuture;
use nix::sys::signal::{Signal, kill, killpg};
use nix::unistd::Pid;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::AbortHandle;

use crate::rpc::RpcError;
use crate::rpc::wire::{self, Incoming};

pub type RequestHandler =
    Arc<dyn Fn(String, Obj) -> BoxFuture<'static, Result<Value, RpcError>> + Send + Sync>;
pub type LogSink = Arc<dyn Fn(String) + Send + Sync>;

pub struct ConnHandlers {
    pub on_request: RequestHandler,
    pub on_log: LogSink,
    pub on_exit: Box<dyn FnOnce(i32) + Send>,
}

pub const SHUTDOWN_GRACE: Duration = Duration::from_millis(800);
pub const OUTBOX_LINES: usize = 1024;

type Pending = Mutex<HashMap<i64, oneshot::Sender<Result<Value, RpcError>>>>;

pub struct Connection {
    pid: u32,
    reaped: AtomicBool,
    out: Mutex<Option<mpsc::Sender<String>>>,
    readers: Mutex<Vec<AbortHandle>>,
    log: LogSink,
    pending: Pending,
    next: AtomicI64,
    ready: AtomicBool,
    exit: watch::Receiver<Option<i32>>,
}

impl Connection {
    pub fn spawn(
        exe: &Path,
        args: &[String],
        cwd: &Path,
        env: &[(String, String)],
        handlers: ConnHandlers,
    ) -> io::Result<Arc<Connection>> {
        let mut child = Command::new(exe)
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .kill_on_drop(true)
            .spawn()?;
        let pid = child.id().unwrap_or_default();
        let (tx, rx) = mpsc::channel::<String>(OUTBOX_LINES);
        let (exit_tx, exit_rx) = watch::channel(None);
        let conn = Arc::new(Connection {
            pid,
            reaped: AtomicBool::new(false),
            out: Mutex::new(Some(tx)),
            readers: Mutex::new(Vec::new()),
            log: handlers.on_log.clone(),
            pending: Mutex::new(HashMap::new()),
            next: AtomicI64::new(1),
            ready: AtomicBool::new(false),
            exit: exit_rx,
        });
        if let Some(stdin) = child.stdin.take() {
            tokio::spawn(write_loop(stdin, rx));
        }
        if let Some(stderr) = child.stderr.take() {
            let log = handlers.on_log.clone();
            let task = tokio::spawn(read_lines(stderr, move |l| log(format!("stderr: {l}"))));
            conn.readers.lock().unwrap().push(task.abort_handle());
        }
        let reader = child.stdout.take().map(|stdout| {
            let c = conn.clone();
            let request = handlers.on_request.clone();
            let log = handlers.on_log.clone();
            tokio::spawn(read_lines(stdout, move |l| c.line(&l, &request, &log)))
        });
        if let Some(reader) = &reader {
            conn.readers.lock().unwrap().push(reader.abort_handle());
        }
        let c = conn.clone();
        tokio::spawn(async move {
            let code = wait_code(&mut child).await;
            c.reaped.store(true, Ordering::SeqCst);
            if let Some(reader) = reader {
                let _ = tokio::time::timeout(Duration::from_millis(300), reader).await;
            }
            c.ready.store(false, Ordering::SeqCst);
            let _ = exit_tx.send(Some(code));
            let waiting: Vec<_> = c.pending.lock().unwrap().drain().map(|(_, tx)| tx).collect();
            for tx in waiting {
                let _ = tx.send(Err(RpcError::exited()));
            }
            (handlers.on_exit)(code);
        });
        Ok(conn)
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn alive(&self) -> bool {
        self.exit.borrow().is_none()
    }

    pub fn exit_code(&self) -> Option<i32> {
        *self.exit.borrow()
    }

    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    fn send(&self, line: String) {
        let full = match self.out.lock().unwrap().as_ref() {
            Some(tx) => matches!(tx.try_send(line), Err(mpsc::error::TrySendError::Full(_))),
            None => false,
        };
        if full {
            (self.log)(format!("o plugin não está lendo o stdin: {OUTBOX_LINES} mensagens na fila, descartei uma"));
        }
    }

    pub fn notify(&self, method: &str, params: Option<Value>) {
        if self.alive() {
            self.send(wire::notification(method, params));
        }
    }

    pub async fn request(
        &self,
        method: &str,
        params: Option<Value>,
        timeout: Duration,
    ) -> Result<Value, RpcError> {
        if !self.alive() {
            return Err(RpcError::not_running());
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        if !self.alive() {
            self.pending.lock().unwrap().remove(&id);
            return Err(RpcError::exited());
        }
        self.send(wire::request(id, method, params));
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_)) => Err(RpcError::exited()),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err(RpcError::timeout(method, timeout.as_secs()))
            }
        }
    }

    fn line(self: &Arc<Self>, line: &str, request: &RequestHandler, log: &LogSink) {
        match wire::parse(line) {
            Incoming::Ignored => {}
            Incoming::Garbage => log(format!("stdout: {line}")),
            Incoming::Notification { method, params } => {
                let fut = request(method, params);
                tokio::spawn(async move {
                    let _ = fut.await;
                });
            }
            Incoming::Request { id, method, params } => {
                let fut = request(method, params);
                let me = self.clone();
                tokio::spawn(async move {
                    let outcome = fut.await;
                    me.send(wire::response(id, outcome));
                });
            }
            Incoming::Response { id, outcome } => {
                if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
                    let _ = tx.send(outcome);
                }
            }
        }
    }

    async fn wait_exit(&self, within: Duration) -> bool {
        let mut rx = self.exit.clone();
        matches!(tokio::time::timeout(within, rx.wait_for(Option::is_some)).await, Ok(Ok(_)))
    }

    fn signal(&self, sig: Signal) {
        if self.reaped.load(Ordering::SeqCst) {
            return;
        }
        if let Ok(pid) = i32::try_from(self.pid) {
            let pid = Pid::from_raw(pid);
            if killpg(pid, sig).is_err() {
                let _ = kill(pid, sig);
            }
        }
    }

    fn stop_readers(&self) {
        for reader in self.readers.lock().unwrap().drain(..) {
            reader.abort();
        }
    }

    pub async fn close(&self, grace: Duration) {
        if !self.alive() {
            return;
        }
        self.notify("shutdown", None);
        self.out.lock().unwrap().take();
        if !self.wait_exit(grace).await {
            self.signal(Signal::SIGTERM);
            if !self.wait_exit(grace).await {
                self.signal(Signal::SIGKILL);
                self.wait_exit(grace).await;
            }
        }
        self.stop_readers();
    }

    pub fn kill(&self) {
        self.out.lock().unwrap().take();
        self.signal(Signal::SIGKILL);
        self.stop_readers();
    }
}

async fn wait_code(child: &mut Child) -> i32 {
    match child.wait().await {
        Ok(status) => status.code().or_else(|| status.signal().map(|s| -s)).unwrap_or(-1),
        Err(_) => -1,
    }
}

async fn write_loop(mut stdin: tokio::process::ChildStdin, mut rx: mpsc::Receiver<String>) {
    while let Some(mut line) = rx.recv().await {
        line.push('\n');
        if stdin.write_all(line.as_bytes()).await.is_err() || stdin.flush().await.is_err() {
            break;
        }
    }
}

async fn read_lines<R: AsyncRead + Unpin>(stream: R, mut each: impl FnMut(String)) {
    let mut reader = BufReader::new(stream);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buf);
                each(text.trim_end_matches(['\n', '\r']).to_owned());
            }
        }
    }
}
