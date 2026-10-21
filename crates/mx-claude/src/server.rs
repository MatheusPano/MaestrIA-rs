use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use percent_encoding::percent_decode_str;
use serde_json::{Map, Value};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::launch::{mcp_config_for, settings_for};
use crate::mcp::{self, NoSink, SuggestionSink};

pub const STATUS_LINE_SCRIPT: &str = include_str!("statusline.sh");

#[derive(Debug, Clone, PartialEq)]
pub enum ServerEvent {
    Hook {
        tab_id: String,
        name: String,
        payload: Map<String, Value>,
    },
    Status {
        tab_id: String,
        payload: Map<String, Value>,
    },
}

pub fn port_file(state_dir: &Path) -> PathBuf {
    state_dir.join("hook-port")
}

pub fn status_line_file(state_dir: &Path) -> PathBuf {
    state_dir.join("statusline.sh")
}

type SharedSink = Arc<RwLock<Arc<dyn SuggestionSink>>>;

#[derive(Clone)]
struct Shared {
    events: UnboundedSender<ServerEvent>,
    sink: SharedSink,
}

pub struct HookServer {
    port: u16,
    state_dir: PathBuf,
    sink: SharedSink,
    task: Option<JoinHandle<()>>,
}

impl HookServer {
    pub async fn start(state_dir: &Path, events: UnboundedSender<ServerEvent>) -> io::Result<Self> {
        let listener = bind(state_dir).await?;
        let port = listener.local_addr()?.port();
        remember(state_dir, port);
        write_status_line(state_dir);

        let sink: SharedSink = Arc::new(RwLock::new(Arc::new(NoSink)));
        let shared = Shared {
            events,
            sink: sink.clone(),
        };
        let app = Router::new()
            .fallback(route)
            .layer(DefaultBodyLimit::disable())
            .with_state(shared);
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Ok(Self {
            port,
            state_dir: state_dir.to_owned(),
            sink,
            task: Some(task),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn running(&self) -> bool {
        self.task.is_some()
    }

    pub fn set_sink(&self, sink: Arc<dyn SuggestionSink>) {
        *self.sink.write().unwrap_or_else(|e| e.into_inner()) = sink;
    }

    pub async fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
        self.port = 0;
    }

    pub fn status_line_file(&self) -> PathBuf {
        status_line_file(&self.state_dir)
    }

    pub fn settings_for(&self, tab_id: &str, cwd: Option<&str>, home: Option<&str>) -> String {
        settings_for(self.port, &self.status_line_file(), tab_id, cwd, home)
    }

    pub fn mcp_config_for(&self, tab_id: &str) -> String {
        mcp_config_for(self.port, tab_id)
    }
}

impl Drop for HookServer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn saved(state_dir: &Path) -> Option<u16> {
    let text = std::fs::read_to_string(port_file(state_dir)).ok()?;
    let port: u32 = text.trim().parse().ok()?;
    (port > 1024 && port < 65536).then_some(port as u16)
}

async fn bind(state_dir: &Path) -> io::Result<TcpListener> {
    if let Some(preferred) = saved(state_dir)
        && let Ok(listener) = TcpListener::bind(("127.0.0.1", preferred)).await
    {
        return Ok(listener);
    }
    TcpListener::bind(("127.0.0.1", 0)).await
}

fn remember(state_dir: &Path, port: u16) {
    if saved(state_dir) == Some(port) {
        return;
    }
    let _ = std::fs::create_dir_all(state_dir);
    let _ = std::fs::write(port_file(state_dir), format!("{port}\n"));
}

fn write_status_line(state_dir: &Path) {
    let _ = std::fs::create_dir_all(state_dir);
    let _ = std::fs::write(status_line_file(state_dir), STATUS_LINE_SCRIPT);
}

fn segments(path: &str) -> Vec<String> {
    let path = path.strip_prefix('/').unwrap_or(path);
    if path.is_empty() {
        return Vec::new();
    }
    path.split('/')
        .map(|s| percent_decode_str(s).decode_utf8_lossy().into_owned())
        .collect()
}

fn json_response(status: StatusCode, body: String) -> Response {
    let mut res = (status, body).into_response();
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    res
}

fn empty(status: StatusCode) -> Response {
    status.into_response()
}

async fn route(State(shared): State<Shared>, method: Method, uri: Uri, body: Bytes) -> Response {
    let segments = segments(uri.path());
    let tab_id = segments.get(1).cloned().unwrap_or_default();
    let first = segments.first().map(String::as_str);
    if first == Some("mcp") {
        return mcp_route(&shared, &method, &tab_id, &body).await;
    }
    if let Some(payload) = payload_of(&body) {
        let name = payload
            .get("hook_event_name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if first == Some("status") {
            if !tab_id.is_empty() {
                let _ = shared.events.send(ServerEvent::Status { tab_id, payload });
            }
        } else if !tab_id.is_empty() && !name.is_empty() {
            let _ = shared.events.send(ServerEvent::Hook {
                tab_id,
                name,
                payload,
            });
        }
    }
    json_response(StatusCode::OK, "{}".to_owned())
}

fn payload_of(body: &[u8]) -> Option<Map<String, Value>> {
    let text = std::str::from_utf8(body).ok()?;
    if text.is_empty() {
        return Some(Map::new());
    }
    match serde_json::from_str(text).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

async fn mcp_route(shared: &Shared, method: &Method, tab_id: &str, body: &[u8]) -> Response {
    if method != Method::POST {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let sink = shared
        .sink
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    match mcp::handle_body(body, tab_id, sink.as_ref()).await {
        None => empty(StatusCode::ACCEPTED),
        Some(answer) => json_response(StatusCode::OK, answer.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::segments;

    #[test]
    fn path_segments_follow_dart_uri_semantics() {
        assert!(segments("/").is_empty());
        assert_eq!(segments("/hook/tab1"), ["hook", "tab1"]);
        assert_eq!(segments("/hook"), ["hook"]);
        assert_eq!(segments("/hook/tab%201/"), ["hook", "tab 1", ""]);
    }
}
