use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value};

use super::Plugins;
use crate::process::{ConnHandlers, Connection, SHUTDOWN_GRACE, resolve_executable};
use crate::rpc::RpcError;
use crate::{BLOCKS_VERSION, PLUGIN_API};

pub const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

impl Plugins {
    pub fn startup(&self) {
        let ids: Vec<String> = self
            .reg()
            .all
            .iter()
            .filter(|e| e.active() && e.manifest.as_ref().is_some_and(|m| m.activates_on("onStartup")))
            .map(|e| e.id())
            .collect();
        for id in ids {
            self.start_detached(&id);
        }
    }

    pub(crate) fn start_detached(&self, id: &str) {
        let me = self.clone();
        let id = id.to_owned();
        self.spawn(async move {
            me.start(&id).await;
        });
    }

    pub async fn start(&self, id: &str) -> Option<Arc<Connection>> {
        let (lock, seen) = {
            let reg = self.reg();
            let e = reg.entry(id)?;
            if let Some(c) = e.live_conn() {
                return Some(c);
            }
            (e.start_lock.clone(), e.attempts)
        };
        let _guard = lock.lock().await;
        let generation = {
            let mut reg = self.reg();
            let e = reg.entry_mut(id)?;
            if let Some(c) = e.live_conn() {
                return Some(c);
            }
            if e.attempts != seen {
                return None;
            }
            e.attempts += 1;
            let m = e.manifest.clone()?;
            if !m.has_process() || !e.enabled {
                return None;
            }
            e.starting = true;
            e.generation
        };
        self.changed();
        let out = self.spawn_process(id, generation).await;
        if let Some(e) = self.reg().entry_mut(id) {
            e.starting = false;
        }
        self.changed();
        out
    }

    async fn spawn_process(&self, id: &str, generation: u64) -> Option<Arc<Connection>> {
        let (m, dir) = {
            let reg = self.reg();
            let e = reg.entry(id)?;
            (e.manifest.clone()?, e.dir.clone())
        };
        let main = m.main.clone().unwrap_or_default();
        let data_dir = self.data_dir(id);
        let mut env = self.inner.env.read().unwrap().clone();
        env.retain(|(k, _)| !k.starts_with("MAESTRIA_PLUGIN_") && k != "MAESTRIA_API");
        env.push(("MAESTRIA_API".into(), PLUGIN_API.to_string()));
        env.push(("MAESTRIA_PLUGIN_ID".into(), id.to_owned()));
        env.push(("MAESTRIA_PLUGIN_DIR".into(), dir.to_string_lossy().into_owned()));
        env.push(("MAESTRIA_PLUGIN_DATA".into(), data_dir.to_string_lossy().into_owned()));
        let path = env.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.clone()).unwrap_or_default();
        let Some(exe) = resolve_executable(&main[0], &dir, &path) else {
            self.fail(id, &format!("não achei \"{}\" — nem na pasta do plugin, nem no PATH", main[0]));
            return None;
        };
        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            self.fail(id, &e.to_string());
            return None;
        }
        let slot: Arc<std::sync::OnceLock<std::sync::Weak<Connection>>> = Arc::default();
        let mut attempt = 0;
        let conn = loop {
            match Connection::spawn(&exe, &main[1..], &dir, &env, self.handlers(id, slot.clone())) {
                Ok(c) => break c,
                // ETXTBSY: a just-written script whose fd leaked into a concurrent fork; transient.
                Err(e) if e.raw_os_error() == Some(nix::libc::ETXTBSY) && attempt < 10 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(e) => {
                    self.fail(id, &format!("não deu pra rodar: {e}"));
                    return None;
                }
            }
        };
        let _ = slot.set(Arc::downgrade(&conn));
        {
            let mut reg = self.reg();
            match reg.entry_mut(id) {
                Some(e) if e.generation == generation => e.conn = Some(conn.clone()),
                _ => {
                    drop(reg);
                    conn.kill();
                    return None;
                }
            }
        }
        self.note(id, &format!("processo {} subiu: {}", conn.pid(), main.join(" ")));
        let params = self.initialize_params(id, &m, &dir, &data_dir);
        match conn.request("initialize", Some(params), INITIALIZE_TIMEOUT).await {
            Ok(_) => {
                {
                    let mut reg = self.reg();
                    let e = reg.entry_mut(id)?;
                    if e.generation != generation || !e.conn.as_ref().is_some_and(|c| Arc::ptr_eq(c, &conn)) {
                        return None;
                    }
                    conn.set_ready();
                    e.crash = None;
                }
                self.changed();
                let me = self.clone();
                let ready_id = id.to_owned();
                self.spawn(async move {
                    tokio::task::yield_now().await;
                    let l = me.inner.listener.read().unwrap().clone();
                    if let Some(l) = l {
                        l.ready(&ready_id);
                    }
                });
                Some(conn)
            }
            Err(e) => {
                self.fail(id, &format!("o initialize falhou: {}", e.message));
                None
            }
        }
    }

    fn initialize_params(
        &self,
        id: &str,
        m: &crate::manifest::PluginManifest,
        dir: &std::path::Path,
        data_dir: &std::path::Path,
    ) -> Value {
        let mut p = Map::new();
        p.insert("apiVersion".into(), Value::from(PLUGIN_API));
        p.insert("blocks".into(), Value::from(BLOCKS_VERSION));
        for (k, v) in [("rfw", 2), ("floats", 1), ("input", 1), ("form", 1), ("modal", 1)] {
            p.insert(k.into(), Value::from(v));
        }
        p.insert("pluginId".into(), Value::from(id));
        p.insert("pluginDir".into(), Value::from(dir.to_string_lossy().into_owned()));
        p.insert("dataDir".into(), Value::from(data_dir.to_string_lossy().into_owned()));
        p.insert("permissions".into(), m.permissions.iter().map(|p| Value::from(p.id())).collect());
        p.insert("settings".into(), Value::Object(self.settings_of(id)));
        Value::Object(p)
    }

    fn handlers(&self, id: &str, slot: Arc<std::sync::OnceLock<std::sync::Weak<Connection>>>) -> ConnHandlers {
        let weak = Arc::downgrade(&self.inner);
        let pid = id.to_owned();
        let on_request: crate::process::RequestHandler = Arc::new(move |method, params| {
            match weak.upgrade() {
                Some(inner) => Plugins { inner }.call(&pid, method, params),
                None => Box::pin(async { Err(RpcError::window_not_ready()) }),
            }
        });
        let weak = Arc::downgrade(&self.inner);
        let pid = id.to_owned();
        let on_log: crate::process::LogSink = Arc::new(move |line| {
            if let Some(inner) = weak.upgrade() {
                Plugins { inner }.note(&pid, &line);
            }
        });
        let weak = Arc::downgrade(&self.inner);
        let pid = id.to_owned();
        let on_exit = Box::new(move |code: i32| {
            let Some(inner) = weak.upgrade() else { return };
            let me = Plugins { inner };
            let mine = slot.get().and_then(std::sync::Weak::upgrade);
            let why = format!("o processo saiu (código {code})");
            {
                let mut reg = me.reg();
                let Some(e) = reg.entry_mut(&pid) else { return };
                let same = matches!((&e.conn, &mine), (Some(a), Some(b)) if Arc::ptr_eq(a, b));
                if !same {
                    return;
                }
                e.conn = None;
                e.crash = Some(why.clone());
                reg.unbusy(&pid);
            }
            me.note(&pid, &why);
            me.changed();
        });
        ConnHandlers { on_request, on_log, on_exit }
    }

    fn fail(&self, id: &str, why: &str) {
        let conn = {
            let mut reg = self.reg();
            let Some(e) = reg.entry_mut(id) else { return };
            e.crash = Some(why.to_owned());
            e.conn.take()
        };
        if let Some(c) = conn {
            c.kill();
        }
        self.note(id, why);
        self.changed();
    }

    pub async fn stop(&self, id: &str) {
        let conn = {
            let mut reg = self.reg();
            let Some(e) = reg.entry_mut(id) else { return };
            e.starting = false;
            e.generation += 1;
            let conn = e.conn.take();
            reg.unbusy(id);
            conn
        };
        if let Some(c) = conn {
            c.close(SHUTDOWN_GRACE).await;
            self.note(id, "processo parado");
        }
    }

    pub async fn stop_all(&self) {
        let ids: Vec<String> = self.reg().all.iter().map(|e| e.id()).collect();
        let mut set = tokio::task::JoinSet::new();
        for id in ids {
            let me = self.clone();
            set.spawn(async move { me.stop(&id).await });
        }
        while set.join_next().await.is_some() {}
    }

    pub fn kill_all(&self) {
        let conns: Vec<_> = self.reg().all.iter_mut().filter_map(|e| e.conn.take()).collect();
        for c in conns {
            c.kill();
        }
    }

    pub async fn set_enabled(&self, id: &str, on: bool) {
        {
            let mut reg = self.reg();
            let Some(e) = reg.entry_mut(id) else { return };
            if e.enabled == on {
                return;
            }
            e.enabled = on;
            if on { reg.disabled.remove(id) } else { reg.disabled.insert(id.to_owned()) };
        }
        if !on {
            self.stop(id).await;
        }
        if let Some(e) = self.reg().entry_mut(id) {
            e.crash = None;
        }
        self.reload_themes();
        self.changed();
        if on && self.get(id).and_then(|p| p.manifest).is_some_and(|m| m.activates_on("onStartup")) {
            self.start_detached(id);
        }
    }

    pub async fn restart(&self, id: &str) {
        self.stop(id).await;
        if let Some(e) = self.reg().entry_mut(id) {
            e.crash = None;
        }
        self.scan();
        if self.get(id).is_some_and(|p| p.active() && p.has_process()) {
            self.start(id).await;
        }
    }
}

