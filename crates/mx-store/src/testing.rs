use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use chrono::TimeDelta;
use mx_core::ports::{BoxFuture, Clock, HookServer, Notifier, PluginBus, Term, TermFactory};
use mx_core::text::Time;
use serde_json::Value;

pub struct FakeClock(Mutex<Time>);

impl FakeClock {
    pub fn new(at: Time) -> FakeClock {
        FakeClock(Mutex::new(at))
    }

    pub fn advance(&self, by: TimeDelta) {
        *self.0.lock().unwrap() += by;
    }

    pub fn set(&self, at: Time) {
        *self.0.lock().unwrap() = at;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Time {
        *self.0.lock().unwrap()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TermLog {
    pub commands: Vec<String>,
    pub displays: Vec<Option<String>>,
    pub shells: Vec<String>,
    pub remarks: Vec<String>,
    pub submitted: Vec<String>,
    pub kills: usize,
    pub hang_ups: usize,
    pub pid: Option<u32>,
    pub exited: bool,
    pub code: Option<i32>,
}

pub struct FakeTerm(Arc<Mutex<TermLog>>);

impl FakeTerm {
    fn log(&self) -> std::sync::MutexGuard<'_, TermLog> {
        self.0.lock().unwrap()
    }
}

impl Term for FakeTerm {
    fn start_shell(&mut self, cwd: &str) {
        let mut l = self.log();
        l.shells.push(cwd.to_owned());
        l.exited = false;
    }

    fn start_command(&mut self, command: &str, _cwd: &str, display: Option<&str>) {
        let mut l = self.log();
        l.commands.push(command.to_owned());
        l.displays.push(display.map(str::to_owned));
        l.exited = false;
    }

    fn park(&mut self) {
        self.log().exited = true;
    }

    fn remark(&mut self, text: &str) {
        self.log().remarks.push(text.to_owned());
    }

    fn kill(&mut self) -> BoxFuture<'static, ()> {
        let mut l = self.log();
        l.kills += 1;
        l.exited = true;
        l.pid = None;
        Box::pin(async {})
    }

    fn hang_up(&mut self) {
        self.log().hang_ups += 1;
    }

    fn relaunch(&mut self, command: &str, cwd: &str) {
        if self.log().exited {
            self.start_command(command, cwd, None);
        }
    }

    fn submit(&mut self, text: &str) {
        let mut l = self.log();
        if !l.exited && !text.trim().is_empty() {
            l.submitted.push(text.to_owned());
        }
    }

    fn pid(&self) -> Option<u32> {
        self.log().pid
    }

    fn exited(&self) -> bool {
        self.log().exited
    }

    fn exit_code(&self) -> Option<i32> {
        self.log().code
    }
}

#[derive(Default)]
pub struct FakeTerms {
    logs: Mutex<HashMap<String, Arc<Mutex<TermLog>>>>,
    next_pid: AtomicU32,
}

impl FakeTerms {
    pub fn log(&self, tab: &str) -> TermLog {
        self.logs
            .lock()
            .unwrap()
            .get(tab)
            .map(|l| l.lock().unwrap().clone())
            .unwrap_or_default()
    }

    pub fn edit(&self, tab: &str, f: impl FnOnce(&mut TermLog)) {
        if let Some(l) = self.logs.lock().unwrap().get(tab) {
            f(&mut l.lock().unwrap());
        }
    }

    pub fn give_pid(&self, tab: &str) -> u32 {
        let pid = 1000 + self.next_pid.fetch_add(1, Ordering::Relaxed);
        self.edit(tab, |l| l.pid = Some(pid));
        pid
    }

    pub fn exit(&self, tab: &str, code: i32) {
        self.edit(tab, |l| {
            l.exited = true;
            l.code = Some(code);
            l.pid = None;
        });
    }
}

impl TermFactory for FakeTerms {
    fn create(&self, tab_id: &str) -> Box<dyn Term> {
        let log = Arc::new(Mutex::new(TermLog::default()));
        self.logs.lock().unwrap().insert(tab_id.to_owned(), log.clone());
        Box::new(FakeTerm(log))
    }
}

#[derive(Default)]
pub struct FakeNotifier {
    pub alerts: Mutex<Vec<(String, String)>>,
    pub badges: Mutex<Vec<Option<String>>>,
    pub links: Mutex<Vec<String>>,
    pub looks: Mutex<Vec<String>>,
    pub reveals: Mutex<Vec<String>>,
    pub chosen: Mutex<Option<String>>,
}

impl Notifier for FakeNotifier {
    fn alert(&self, title: &str, body: &str) {
        self.alerts.lock().unwrap().push((title.to_owned(), body.to_owned()));
    }

    fn badge(&self, label: Option<&str>) {
        self.badges.lock().unwrap().push(label.map(str::to_owned));
    }

    fn open_link(&self, url: &str) {
        self.links.lock().unwrap().push(url.to_owned());
    }

    fn quick_look(&self, path: &str) {
        self.looks.lock().unwrap().push(path.to_owned());
    }

    fn reveal(&self, path: &str) {
        self.reveals.lock().unwrap().push(path.to_owned());
    }

    fn choose_markdown(&self, _start_in: Option<&str>) -> BoxFuture<'static, Option<String>> {
        let chosen = self.chosen.lock().unwrap().clone();
        Box::pin(async move { chosen })
    }
}

#[derive(Default)]
pub struct FakeBus {
    pub events: Mutex<Vec<(String, Value)>>,
}

impl FakeBus {
    pub fn names(&self) -> Vec<String> {
        self.events.lock().unwrap().iter().map(|(n, _)| n.clone()).collect()
    }
}

impl PluginBus for FakeBus {
    fn emit(&self, event: &str, data: Value, _activations: &[&str], _permission: Option<&str>) {
        self.events.lock().unwrap().push((event.to_owned(), data));
    }
}

pub struct FakeHooks {
    pub port: u16,
}

impl HookServer for FakeHooks {
    fn port(&self) -> u16 {
        self.port
    }

    fn mcp_config_for(&self, tab_id: &str) -> String {
        mx_claude::mcp_config_for(self.port, tab_id)
    }

    fn settings_for(&self, tab_id: &str, cwd: &str) -> String {
        format!("{{\"tab\":\"{tab_id}\",\"cwd\":\"{cwd}\"}}")
    }
}
