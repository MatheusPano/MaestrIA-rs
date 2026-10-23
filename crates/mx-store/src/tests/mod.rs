mod config;
mod features;
mod flows;
mod groups;
mod hibernate;
mod history;
mod layout;
mod notices;
mod pin;
mod plugin_live;
mod plugin_screen;
mod plugins;
mod readers;
mod reorder;
mod rested;
mod search;
mod suggestions;
mod timing;
mod workspace;

use std::sync::Arc;

use chrono::{Local, TimeDelta, TimeZone};
use mx_core::model::{Folder, Tab, TabKind};
use mx_core::text::Time;
use mx_sys::FakeRunner;
use serde_json::{Map, Value, json};
use tempfile::TempDir;

use crate::testing::{FakeBus, FakeClock, FakeHooks, FakeNotifier, FakeTerms};
use crate::{Ports, Store, StoreOptions};

pub(crate) struct Rig {
    pub s: Store,
    pub clock: Arc<FakeClock>,
    pub terms: Arc<FakeTerms>,
    pub notifier: Arc<FakeNotifier>,
    pub bus: Arc<FakeBus>,
    pub runner: Arc<FakeRunner>,
    pub dir: TempDir,
    _rt: Option<tokio::runtime::Runtime>,
}

pub(crate) fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> Time {
    Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
}

pub(crate) fn fixtures() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

impl Rig {
    pub fn new() -> Rig {
        Rig::with_runner(FakeRunner::new())
    }

    pub fn with_runner(runner: FakeRunner) -> Rig {
        let (handle, rt) = match tokio::runtime::Handle::try_current() {
            Ok(h) => (h, None),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                (rt.handle().clone(), Some(rt))
            }
        };
        let dir = tempfile::tempdir().unwrap();
        let clock = Arc::new(FakeClock::new(at(2026, 9, 18, 12, 0)));
        let terms = Arc::new(FakeTerms::default());
        let notifier = Arc::new(FakeNotifier::default());
        let bus = Arc::new(FakeBus::default());
        let runner = Arc::new(runner);
        let ports = Ports {
            clock: clock.clone(),
            terms: terms.clone(),
            runner: runner.clone(),
            notifier: notifier.clone(),
            plugins: bus.clone(),
            hooks: Arc::new(FakeHooks { port: 4242 }),
            rt: handle,
        };
        let opts = StoreOptions::at(
            dir.path().join("state"),
            fixtures().join("history"),
            Some(dir.path().to_str().unwrap()),
        );
        Rig { s: Store::new(opts, ports), clock, terms, notifier, bus, runner, dir, _rt: rt }
    }

    pub fn now(&self) -> Time {
        self.s.now()
    }

    pub fn advance(&mut self, by: TimeDelta) {
        self.clock.advance(by);
        self.s.fire_due();
    }

    pub fn folder(&mut self, root: &str, name: &str) -> Folder {
        let f = Folder::new(root, name);
        self.s.folders.push(f.clone());
        f
    }

    pub fn s1(&mut self) -> Folder {
        self.folder("/repo", "meu-repo")
    }

    pub fn add_tab(&mut self, id: &str, root: &str, kind: TabKind, cwd: &str, label: Option<&str>) {
        let folder = self.s.find_folder(root).cloned().unwrap_or_else(|| Folder::new(root, root));
        let mut tab = Tab::new(id, &folder, kind, cwd, self.now());
        tab.custom_label = label.map(str::to_owned);
        let term = self.s.p.terms.create(id);
        self.s.tabs.push(tab);
        self.s.terms.insert(id.to_owned(), term);
    }

    pub fn panel(&mut self, name: &str) -> String {
        self.add_tab(name, "/repo", TabKind::Claude, "/repo", Some(name));
        name.to_owned()
    }

    pub fn shell(&mut self, name: &str) -> String {
        self.add_tab(name, "/repo", TabKind::Shell, "/repo", Some(name));
        name.to_owned()
    }

    pub fn hook(&mut self, id: &str, name: &str, payload: Value) {
        let map: Map<String, Value> = payload.as_object().cloned().unwrap_or_default();
        self.s.apply_hook(id, name, map);
    }

    pub fn turn(&mut self, id: &str) {
        self.hook(id, "UserPromptSubmit", json!({}));
        self.hook(id, "Stop", json!({}));
    }

    pub fn silent(&mut self, id: &str, by: TimeDelta) {
        let now = self.now();
        self.s.tab_mut(id).unwrap().hooks.last_event_at = Some(now - by);
    }

    pub fn t(&self, id: &str) -> &Tab {
        self.s.tab(id).unwrap()
    }

    pub fn tm(&mut self, id: &str) -> &mut Tab {
        self.s.tab_mut(id).unwrap()
    }

    pub fn ids(&self) -> Vec<String> {
        self.s.tabs.iter().map(|t| t.id.clone()).collect()
    }

    pub fn written(&mut self) -> Value {
        self.advance(TimeDelta::milliseconds(400));
        let text = std::fs::read_to_string(self.s.opts.config_path()).expect("config written");
        serde_json::from_str(&text).unwrap()
    }

    pub fn mkdir(&self, name: &str) -> String {
        let p = self.dir.path().join(name);
        std::fs::create_dir_all(&p).unwrap();
        p.to_str().unwrap().to_owned()
    }
}

pub(crate) fn names(list: &[&Folder]) -> Vec<String> {
    list.iter().map(|f| f.name.clone()).collect()
}
