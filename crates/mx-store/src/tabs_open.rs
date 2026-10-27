use chrono::TimeDelta;
use mx_claude::ClaudeLaunch;
use mx_core::model::{ClaudeStatus, Doc, Folder, Setup, Tab, TabKind};
use mx_core::ports::Term;
use mx_core::text::last_segment;
use serde_json::json;

use crate::changes::Areas;
use crate::msg::Done;
use crate::store::Store;
use crate::timers::{SETTLE, TimerKey};

pub const HIBERNATED_AT_BIRTH: &str = "Hibernada — a conversa volta com um clique";
const EARLY_EXIT: TimeDelta = TimeDelta::seconds(5);
const TRUST_RETRY: TimeDelta = TimeDelta::seconds(2);
const TRUST_TRIES: u32 = 300;

#[derive(Debug, Clone)]
pub struct ClaudeSpec {
    pub folder: Folder,
    pub cwd: String,
    pub label: Option<String>,
    pub resume_id: Option<String>,
    pub feature: Option<String>,
    pub prompt: Option<String>,
    pub start: bool,
}

impl ClaudeSpec {
    pub fn new(folder: &Folder, cwd: &str) -> ClaudeSpec {
        ClaudeSpec {
            folder: folder.clone(),
            cwd: cwd.to_owned(),
            label: None,
            resume_id: None,
            feature: None,
            prompt: None,
            start: true,
        }
    }
}

impl Store {
    fn attachable(&self, folder: &Folder, feature: Option<&str>) -> Option<String> {
        let id = feature?;
        let f = self.feature_by_id(id)?;
        (f.folder_root == folder.root).then(|| id.to_owned())
    }

    pub fn open_shell(
        &mut self,
        folder: &Folder,
        cwd: Option<&str>,
        command: Option<&str>,
        feature: Option<&str>,
        launcher: Option<&str>,
    ) -> String {
        self.open_shell_with(folder, cwd, command, feature, launcher, true)
    }

    pub fn open_shell_with(
        &mut self,
        folder: &Folder,
        cwd: Option<&str>,
        command: Option<&str>,
        feature: Option<&str>,
        launcher: Option<&str>,
        place: bool,
    ) -> String {
        let cwd = cwd.unwrap_or(&folder.root).to_owned();
        let id = self.next_tab_id();
        let mut tab = Tab::new(&id, folder, TabKind::Shell, &cwd, self.now());
        let launcher = launcher.and_then(|l| self.launcher_by_id(l)).cloned();
        tab.launcher_id = launcher.as_ref().map(|l| l.id.clone());
        tab.feature_or_hotfix_id = self.attachable(folder, feature);
        let term = self.p.terms.create(&id);
        self.register(tab, term, place);
        let run = command
            .map(str::to_owned)
            .or_else(|| launcher.map(|l| l.command));
        if let Some(term) = self.terms.get_mut(&id) {
            match &run {
                None => term.start_shell(&cwd),
                Some(r) => term.start_command(r, &cwd, None),
            }
        }
        self.sync_term(&id);
        id
    }

    pub fn open_launcher(
        &mut self,
        launcher: &str,
        folder: &Folder,
        cwd: Option<&str>,
        feature: Option<&str>,
    ) -> Option<String> {
        self.launcher_by_id(launcher)?;
        Some(self.open_shell(folder, cwd, None, feature, Some(launcher)))
    }

    pub fn open_claude(&mut self, spec: ClaudeSpec) -> String {
        self.open_claude_placed(spec, true)
    }

    pub fn open_claude_placed(&mut self, spec: ClaudeSpec, place: bool) -> String {
        let id = self.next_tab_id();
        let mut tab = Tab::new(&id, &spec.folder, TabKind::Claude, &spec.cwd, self.now());
        tab.custom_label = spec.label.clone();
        tab.session_id = spec.resume_id.clone();
        tab.feature_or_hotfix_id = self.attachable(&spec.folder, spec.feature.as_deref());
        tab.agent_name = Some(match spec.label.as_deref().filter(|l| !l.is_empty()) {
            Some(l) => l.to_owned(),
            None => last_segment(&spec.cwd).to_owned(),
        });
        let term = self.p.terms.create(&id);
        self.register(tab, term, place);
        if spec.start {
            self.launch_claude(&id, spec.resume_id, spec.prompt);
        } else {
            if let Some(term) = self.terms.get_mut(&id) {
                term.park();
                term.remark(HIBERNATED_AT_BIRTH);
            }
            self.sync_term(&id);
            if let Some(t) = self.tab_mut(&id) {
                t.hibernated = true;
                t.hooks.status = ClaudeStatus::Ended;
            }
        }
        let git = self.git.clone();
        let (tab_id, cwd) = (id.clone(), spec.cwd);
        self.spawn(async move {
            let branch = git.branch_of(&cwd).await;
            Done::Branch { tab_id, branch }
        });
        id
    }

    pub(crate) fn launch_claude(&mut self, id: &str, resume_id: Option<String>, prompt: Option<String>) {
        let Some(tab) = self.tab(id) else { return };
        let cwd = tab.cwd.clone();
        let name = tab
            .agent_name
            .clone()
            .unwrap_or_else(|| last_segment(&cwd).to_owned());
        let feature = tab
            .feature_or_hotfix_id
            .as_deref()
            .and_then(|f| self.feature_by_id(f));
        let hooks = &self.p.hooks;
        let launch = ClaudeLaunch {
            port: hooks.port(),
            mcp_config: hooks.mcp_config_for(id),
            settings: hooks.settings_for(id, &cwd),
            name,
            brief: feature.map(|f| f.brief.clone()),
            project: feature.map(|f| f.name.clone()),
            resume_id,
            prompt,
        };
        if let Some(term) = self.terms.get_mut(id) {
            term.start_command(&launch.command_line(), &cwd, Some(&launch.display_line()));
        }
        self.sync_term(id);
        let at = self.now() + SETTLE;
        self.timers.set(TimerKey::Settle(id.to_owned()), at);
        self.plugin.extras.settle_tries.insert(id.to_owned(), TRUST_TRIES);
    }

    pub(crate) fn settle_launch(&mut self, id: &str) {
        let tries = self.plugin.extras.settle_tries.remove(id).unwrap_or(0);
        if self.tab(id).is_none_or(|t| t.exited) {
            return;
        }
        if self.terms.get(id).is_some_and(|term| term.asks_trust()) {
            if tries > 0 {
                self.plugin.extras.settle_tries.insert(id.to_owned(), tries - 1);
                let at = self.now() + TRUST_RETRY;
                self.timers.set(TimerKey::Settle(id.to_owned()), at);
            }
            return;
        }
        if self.tab_mut(id).is_some_and(|t| t.hooks.settle()) {
            self.p.plugins.emit(
                "session.status",
                json!({"tabId": id, "status": ClaudeStatus::Ready.name(), "previous": ClaudeStatus::Starting.name()}),
                &["onSession"],
                None,
            );
            self.mark_tab(id);
        }
    }

    fn register(&mut self, tab: Tab, term: Box<dyn Term>, place: bool) {
        let id = tab.id.clone();
        let data = json!({
            "tabId": id,
            "kind": tab.kind.name(),
            "cwd": tab.cwd,
            "folder": tab.folder_root,
        });
        self.tabs.push(tab);
        self.terms.insert(id.clone(), term);
        self.p.plugins.emit("session.opened", data, &["onSession"], None);
        if place {
            self.place(&id);
        }
        self.save();
        self.mark_tab(&id);
        self.mark(Areas::PANES);
    }

    fn push_passive(&mut self, mut tab: Tab, folder: &Folder, feature: Option<&str>) -> String {
        tab.feature_or_hotfix_id = self.attachable(folder, feature);
        let id = tab.id.clone();
        self.tabs.push(tab);
        self.mark_tab(&id);
        id
    }

    pub(crate) fn new_reader(&mut self, doc: Doc, folder: &Folder, cwd: Option<&str>, feature: Option<&str>) -> String {
        let id = self.next_tab_id();
        let mut tab = Tab::new(&id, folder, TabKind::Reader, cwd.unwrap_or(&folder.root), self.now());
        tab.doc = Some(doc);
        self.push_passive(tab, folder, feature)
    }

    pub(crate) fn new_setup(&mut self, setup: Setup, folder: &Folder, cwd: Option<&str>, feature: Option<&str>) -> String {
        let id = self.next_tab_id();
        let mut tab = Tab::new(&id, folder, TabKind::Setup, cwd.unwrap_or(&folder.root), self.now());
        tab.setup = Some(setup);
        self.push_passive(tab, folder, feature)
    }

    pub fn relaunch(&mut self, id: &str) {
        let Some(tab) = self.tab(id) else { return };
        if !tab.exited {
            return;
        }
        let Some(launcher) = tab.launcher_id.as_deref().and_then(|l| self.launcher_by_id(l)) else {
            return;
        };
        let (command, cwd) = (launcher.command.clone(), tab.cwd.clone());
        if let Some(term) = self.terms.get_mut(id) {
            term.relaunch(&command, &cwd);
        }
        self.sync_term(id);
        self.mark_tab(id);
    }

    pub(crate) fn on_term_exit(&mut self, id: &str, code: Option<i32>) {
        let Some(tab) = self.tab(id) else { return };
        if self.terms.get(id).is_some_and(|t| !t.exited()) {
            return;
        }
        let early = tab.kind == TabKind::Claude && self.now() - tab.started_at < EARLY_EXIT;
        let title = self.title(tab);
        let code = code.or_else(|| self.terms.get(id).and_then(|t| t.exit_code()));
        if let Some(t) = self.tab_mut(id) {
            t.exited = true;
            t.exit_code = code;
            if matches!(t.hooks.status, ClaudeStatus::Starting | ClaudeStatus::Working | ClaudeStatus::Tool) {
                t.hooks.status = ClaudeStatus::Ended;
            }
        }
        if early {
            let text = mx_sys::shell::early_exit_message(&title, code, self.opts.platform);
            self.show_banner_sticky(&text);
        }
        self.p
            .plugins
            .emit("session.exited", json!({"tabId": id, "code": code}), &[], None);
        self.save();
        self.mark_tab(id);
    }

    pub(crate) fn apply_branch(&mut self, id: &str, branch: String) {
        let Some(tab) = self.tab_mut(id) else { return };
        if tab.branch == branch {
            return;
        }
        tab.branch = branch;
        self.mark_tab(id);
    }
}
