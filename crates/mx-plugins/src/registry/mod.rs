mod delivery;
mod install;
mod lifecycle;
mod persist;
mod scan;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use mx_theme::Themes;
use serde_json::Value;
use tokio::runtime::Handle;

pub use install::{InstallError, StagedPlugin, installed_banner, trust_facts, trust_headline};
pub use lifecycle::{CAPABILITIES, MAESTRIA_COMPAT};

use crate::catalog::PluginOrigin;
use crate::chord::KeyPress;
use crate::manifest::{PluginCommand, PluginManifest};
use crate::process::Connection;
use crate::rpc::PluginCalls;
use crate::state::{LOG_CAP, PluginInfo, PluginState, log_line};

pub const MAX_SIDEBAR_COMMANDS: usize = 3;

pub trait PluginListener: Send + Sync {
    fn changed(&self) {}
    fn ready(&self, _plugin_id: &str) {}
    fn logged(&self, _plugin_id: &str) {}
}

pub(crate) struct Entry {
    pub dir: PathBuf,
    pub manifest: Option<Arc<PluginManifest>>,
    pub problem: Option<String>,
    pub linked: bool,
    pub enabled: bool,
    pub crash: Option<String>,
    pub palettes: Vec<String>,
    pub log: VecDeque<String>,
    pub conn: Option<Arc<Connection>>,
    pub starting: bool,
    pub attempts: u64,
    pub generation: u64,
    pub start_lock: Arc<tokio::sync::Mutex<()>>,
}

impl Entry {
    fn new(dir: PathBuf, linked: bool) -> Self {
        Entry {
            dir,
            manifest: None,
            problem: None,
            linked,
            enabled: true,
            crash: None,
            palettes: Vec::new(),
            log: VecDeque::new(),
            conn: None,
            starting: false,
            attempts: 0,
            generation: 0,
            start_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn id(&self) -> String {
        match &self.manifest {
            Some(m) => m.id.clone(),
            None => mx_core::text::last_segment(&self.dir.to_string_lossy()).to_owned(),
        }
    }

    pub fn active(&self) -> bool {
        self.manifest.is_some() && self.enabled
    }

    pub fn live_conn(&self) -> Option<Arc<Connection>> {
        self.conn.clone().filter(|c| c.alive())
    }

    fn note(&mut self, line: &str) {
        self.log.push_back(log_line(line));
        while self.log.len() > LOG_CAP {
            self.log.pop_front();
        }
    }
}

#[derive(Default)]
pub(crate) struct Registry {
    pub all: Vec<Entry>,
    pub disabled: BTreeSet<String>,
    pub settings: serde_json::Map<String, Value>,
    pub busy: BTreeSet<String>,
    pub themes: Themes,
    pub scanned: bool,
    pub origins: BTreeMap<String, PluginOrigin>,
    pub manual_update: BTreeSet<String>,
    pub auto_update_off: bool,
}

impl Registry {
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.all.iter().find(|e| e.id() == id)
    }

    pub fn entry_mut(&mut self, id: &str) -> Option<&mut Entry> {
        self.all.iter_mut().find(|e| e.id() == id)
    }

    pub fn unbusy(&mut self, id: &str) {
        let prefix = format!("{id}/");
        self.busy.retain(|b| !b.starts_with(&prefix));
    }
}

pub(crate) struct Inner {
    pub root: PathBuf,
    pub data_root: PathBuf,
    pub linux: bool,
    pub reg: Mutex<Registry>,
    pub calls: RwLock<Option<Arc<dyn PluginCalls>>>,
    pub listener: RwLock<Option<Arc<dyn PluginListener>>>,
    pub env: RwLock<Vec<(String, String)>>,
    pub runtime: RwLock<Option<Handle>>,
}

#[derive(Clone)]
pub struct Plugins {
    pub(crate) inner: Arc<Inner>,
}

impl Plugins {
    pub fn new(state_dir: &Path) -> Self {
        Self::with_dirs(state_dir.join("plugins"), state_dir.join("plugin-data"))
    }

    pub fn with_dirs(root: PathBuf, data_root: PathBuf) -> Self {
        Plugins {
            inner: Arc::new(Inner {
                root,
                data_root,
                linux: cfg!(target_os = "linux"),
                reg: Mutex::new(Registry::default()),
                calls: RwLock::new(None),
                listener: RwLock::new(None),
                env: RwLock::new(std::env::vars().collect()),
                runtime: RwLock::new(Handle::try_current().ok()),
            }),
        }
    }

    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    pub fn data_dir(&self, id: &str) -> PathBuf {
        self.inner.data_root.join(id)
    }

    pub fn set_calls(&self, calls: Arc<dyn PluginCalls>) {
        *self.inner.calls.write().unwrap() = Some(calls);
    }

    pub fn set_listener(&self, listener: Arc<dyn PluginListener>) {
        *self.inner.listener.write().unwrap() = Some(listener);
    }

    pub fn set_env(&self, env: Vec<(String, String)>) {
        *self.inner.env.write().unwrap() = env;
    }

    pub fn set_runtime(&self, handle: Handle) {
        *self.inner.runtime.write().unwrap() = Some(handle);
    }

    pub(crate) fn reg(&self) -> MutexGuard<'_, Registry> {
        self.inner.reg.lock().unwrap()
    }

    pub(crate) fn spawn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let handle = self.inner.runtime.read().unwrap().clone();
        match handle {
            Some(h) => drop(h.spawn(fut)),
            None => drop(tokio::spawn(fut)),
        }
    }

    pub(crate) fn changed(&self) {
        let l = self.inner.listener.read().unwrap().clone();
        if let Some(l) = l {
            l.changed();
        }
    }

    pub fn note(&self, id: &str, line: &str) {
        if let Some(e) = self.reg().entry_mut(id) {
            e.note(line);
        }
        let l = self.inner.listener.read().unwrap().clone();
        if let Some(l) = l {
            l.logged(id);
        }
    }

    pub(crate) fn info_of(&self, e: &Entry) -> PluginInfo {
        let id = e.id();
        let ready = e.live_conn().is_some_and(|c| c.ready());
        PluginInfo {
            data_dir: self.data_dir(&id),
            state: PluginState::derive(e.manifest.is_some(), e.enabled, ready, e.starting, e.crash.is_some()),
            id,
            dir: e.dir.clone(),
            manifest: e.manifest.clone(),
            problem: e.problem.clone(),
            linked: e.linked,
            enabled: e.enabled,
            crash: e.crash.clone(),
            palettes: e.palettes.clone(),
        }
    }

    pub fn scanned(&self) -> bool {
        self.reg().scanned
    }

    pub fn all(&self) -> Vec<PluginInfo> {
        let reg = self.reg();
        reg.all.iter().map(|e| self.info_of(e)).collect()
    }

    pub fn get(&self, id: &str) -> Option<PluginInfo> {
        let reg = self.reg();
        reg.entry(id).map(|e| self.info_of(e))
    }

    pub fn log_of(&self, id: &str) -> Vec<String> {
        self.reg().entry(id).map(|e| e.log.iter().cloned().collect()).unwrap_or_default()
    }

    pub fn themes(&self) -> Themes {
        self.reg().themes.clone()
    }

    pub fn commands(&self) -> Vec<PluginCommand> {
        self.reg()
            .all
            .iter()
            .filter(|e| e.active())
            .flat_map(|e| e.manifest.as_ref().map(|m| m.commands.clone()).unwrap_or_default())
            .collect()
    }

    pub fn sidebar_commands(&self) -> Vec<PluginCommand> {
        self.commands().into_iter().filter(|c| c.sidebar).take(MAX_SIDEBAR_COMMANDS).collect()
    }

    pub fn command_by_id(&self, full_id: &str) -> Option<PluginCommand> {
        self.commands().into_iter().find(|c| c.full_id() == full_id)
    }

    pub fn command_for(&self, press: &KeyPress, app_keymap_hit: bool) -> Option<PluginCommand> {
        if app_keymap_hit {
            return None;
        }
        let hit = self.commands().into_iter().find(|c| c.key.is_some_and(|k| k.hits(press)))?;
        if let Some(k) = hit.key {
            self.note(&hit.plugin_id, &format!("tecla {} → {}", k.label(self.inner.linux), hit.id));
        }
        Some(hit)
    }

    pub fn busy(&self) -> BTreeSet<String> {
        self.reg().busy.clone()
    }

    pub fn plugin_busy(&self, id: &str) -> bool {
        let prefix = format!("{id}/");
        self.reg().busy.iter().any(|b| b.starts_with(&prefix))
    }

    pub fn set_busy(&self, full_id: &str, on: bool) {
        let changed = {
            let mut reg = self.reg();
            if on { reg.busy.insert(full_id.to_owned()) } else { reg.busy.remove(full_id) }
        };
        if changed {
            self.changed();
        }
    }

    pub fn suggestion_takers(&self) -> Vec<PluginInfo> {
        self.all()
            .into_iter()
            .filter(|p| p.active() && p.manifest.as_ref().is_some_and(|m| m.suggestions.is_some()))
            .collect()
    }

    pub fn is_up(&self, id: &str) -> bool {
        self.reg().entry(id).and_then(Entry::live_conn).is_some_and(|c| c.ready())
    }

    pub fn disabled(&self) -> BTreeSet<String> {
        self.reg().disabled.clone()
    }
}
