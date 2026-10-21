#![allow(dead_code)]

pub mod host;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mx_core::json::Obj;
use mx_core::ports::BoxFuture;
use mx_plugins::{PluginCalls, PluginInfo, Plugins, RpcError};
use serde_json::{Value, json};

pub const SH_PLUGIN: &str = include_str!("../fixtures/sh/sh_plugin.sh");
pub const TAKER: &str = include_str!("../fixtures/sh/taker.sh");
pub const BOARD: &str = include_str!("../fixtures/sh/board.sh");
pub const MORRE: &str = include_str!("../fixtures/sh/morre.sh");
pub const ECHO_ENV: &str = include_str!("../fixtures/sh/echo_env.sh");
pub const DEAF: &str = include_str!("../fixtures/sh/deaf.sh");
pub const SILENT: &str = include_str!("../fixtures/sh/silent.sh");
pub const AURORA: &str = include_str!("../fixtures/tema-e-atalhos/themes/aurora.json");

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn obj(v: Value) -> Obj {
    match v {
        Value::Object(m) => m,
        _ => panic!("not an object"),
    }
}

pub fn write_plugin(parent: &Path, name: &str, manifest: Value, files: &[(&str, &str)]) -> PathBuf {
    let dir = parent.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(mx_plugins::MANIFEST_NAME), manifest.to_string()).unwrap();
    for (path, text) in files {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, text).unwrap();
        if path.ends_with(".sh") {
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    dir
}

pub fn plugins_in(tmp: &Path) -> Plugins {
    Plugins::with_dirs(tmp.join("plugins"), tmp.join("plugin-data"))
}

pub async fn until(mut ok: impl FnMut() -> bool) -> bool {
    for _ in 0..100 {
        if ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    ok()
}

pub fn aurora() -> Value {
    serde_json::from_str(AURORA).unwrap()
}

#[derive(Default)]
pub struct Recorder {
    pub calls: Mutex<Vec<String>>,
    pub answer: Mutex<Value>,
}

impl Recorder {
    pub fn new(answer: Value) -> Arc<Self> {
        Arc::new(Recorder { calls: Mutex::new(Vec::new()), answer: Mutex::new(answer) })
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl PluginCalls for Recorder {
    fn call(&self, _plugin: PluginInfo, method: String, params: Obj) -> BoxFuture<'static, Result<Value, RpcError>> {
        self.calls.lock().unwrap().push(format!("{method} {}", Value::Object(params)));
        let answer = self.answer.lock().unwrap().clone();
        Box::pin(async move { Ok(answer) })
    }
}

pub fn base(extra: Value) -> Value {
    let mut m = json!({"id": "x", "version": "1"});
    for (k, v) in obj(extra) {
        m[k] = v;
    }
    m
}
