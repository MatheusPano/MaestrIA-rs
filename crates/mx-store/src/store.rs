use std::collections::HashMap;

use mx_core::layout::{self, PaneNode};
use mx_core::model::{
    AgentInfo, FeatureOrHotfix, Folder, Launcher, PaneGroup, SuggestionEnd, Tab, TabCtx,
    Workspace, WorktreeInfo, ids,
};
use mx_core::ports::Term;
use mx_core::text::Time;
use mx_sys::code_workspace::WorkspaceImport;
use mx_sys::git::Git;

use crate::changes::{Areas, Changes};
use crate::msg::Background;
use crate::notices::Bell;
use crate::plugins::PluginUi;
use crate::ports::{Ports, StoreOptions};
use crate::prefs::Prefs;
use crate::search::Search;
use crate::suggestions::Tracking;
use crate::tabs_ops::Reveal;
use crate::timers::Timers;

pub struct Store {
    pub(crate) p: Ports,
    pub(crate) opts: StoreOptions,
    pub(crate) git: Git,
    pub(crate) folders: Vec<Folder>,
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) root_order: Vec<String>,
    pub(crate) features: Vec<FeatureOrHotfix>,
    pub(crate) loose: Folder,
    pub(crate) tabs: Vec<Tab>,
    pub(crate) terms: HashMap<String, Box<dyn Term>>,
    pub(crate) groups: Vec<PaneGroup>,
    pub(crate) launchers: Vec<Launcher>,
    pub(crate) worktrees: HashMap<String, Vec<WorktreeInfo>>,
    pub(crate) panes: Option<PaneNode>,
    pub(crate) focused: Option<String>,
    pub(crate) hole: Option<(PaneNode, String)>,
    pub(crate) prefs: Prefs,
    pub(crate) search: Search,
    pub(crate) bell: Bell,
    pub(crate) reveal: Option<Reveal>,
    pub(crate) agents: Vec<AgentInfo>,
    pub(crate) alerted: Vec<String>,
    pub(crate) badge: Option<Option<String>>,
    pub(crate) settled: HashMap<String, SuggestionEnd>,
    pub(crate) tracking: HashMap<String, Tracking>,
    pub(crate) installed_plugins: Vec<String>,
    pub(crate) last_import: Option<WorkspaceImport<String>>,
    pub(crate) ages: Vec<(String, String)>,
    pub(crate) eager: Option<bool>,
    pub(crate) seq: u64,
    pub(crate) run: String,
    pub(crate) restoring: bool,
    pub(crate) quitting: bool,
    pub(crate) gone: bool,
    pub(crate) timers: Timers,
    pub(crate) changes: Changes,
    pub(crate) bg: Background,
    pub(crate) plugin: PluginUi,
    pub(crate) updates: crate::updates::Updates,
}

impl Store {
    pub fn new(opts: StoreOptions, ports: Ports) -> Store {
        let now = ports.clock.now();
        let micros = now.timestamp_micros().max(0) as u64;
        Store {
            git: Git::new(ports.runner.clone()),
            loose: Folder::loose(opts.home.as_deref()),
            updates: crate::updates::Updates::new(&opts.state_dir),
            p: ports,
            opts,
            folders: Vec::new(),
            workspaces: Vec::new(),
            root_order: Vec::new(),
            features: Vec::new(),
            tabs: Vec::new(),
            terms: HashMap::new(),
            groups: Vec::new(),
            launchers: Vec::new(),
            worktrees: HashMap::new(),
            panes: None,
            focused: None,
            hole: None,
            prefs: Prefs::default(),
            search: Search::default(),
            bell: Bell::default(),
            reveal: None,
            agents: Vec::new(),
            alerted: Vec::new(),
            badge: None,
            settled: HashMap::new(),
            tracking: HashMap::new(),
            installed_plugins: Vec::new(),
            last_import: None,
            ages: Vec::new(),
            eager: None,
            seq: 0,
            run: ids::run_stamp(micros),
            restoring: false,
            quitting: false,
            gone: false,
            timers: Timers::default(),
            changes: Changes::default(),
            bg: Background::new(),
            plugin: PluginUi::default(),
        }
    }

    pub fn options(&self) -> &StoreOptions {
        &self.opts
    }

    pub(crate) fn now(&self) -> Time {
        self.p.clock.now()
    }

    pub(crate) fn next_tab_id(&mut self) -> String {
        let id = ids::tab_id(&self.run, self.seq);
        self.seq += 1;
        id
    }

    pub(crate) fn mark(&mut self, areas: Areas) {
        if !self.gone {
            self.changes.mark(areas);
        }
    }

    pub(crate) fn mark_tab(&mut self, id: &str) {
        if !self.gone {
            self.changes.mark_tab(id);
        }
    }

    pub fn take_changes(&mut self) -> Changes {
        self.sync_eager();
        std::mem::take(&mut self.changes)
    }

    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tab(&self, id: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub(crate) fn tab_mut(&mut self, id: &str) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub(crate) fn pos(&self, id: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    pub fn has_tab(&self, id: &str) -> bool {
        self.pos(id).is_some()
    }

    pub fn term(&self, id: &str) -> Option<&dyn Term> {
        self.terms.get(id).map(|t| t.as_ref())
    }

    pub fn term_mut(&mut self, id: &str) -> Option<&mut (dyn Term + 'static)> {
        self.terms.get_mut(id).map(|t| t.as_mut())
    }

    pub(crate) fn sync_term(&mut self, id: &str) {
        let Some((exited, code)) = self.terms.get(id).map(|t| (t.exited(), t.exit_code())) else {
            return;
        };
        if let Some(tab) = self.tab_mut(id) {
            tab.exited = exited;
            tab.exit_code = code;
        }
    }

    pub fn folder_of(&self, t: &Tab) -> &Folder {
        if t.loose {
            return &self.loose;
        }
        self.find_folder(&t.folder_root).unwrap_or(&self.loose)
    }

    pub fn find_folder(&self, root: &str) -> Option<&Folder> {
        self.folders.iter().find(|f| f.root == root)
    }

    pub(crate) fn folder_mut(&mut self, root: &str) -> Option<&mut Folder> {
        self.folders.iter_mut().find(|f| f.root == root)
    }

    pub(crate) fn folder_or_loose(&self, root: &str) -> Option<Folder> {
        if root == self.loose.root {
            return Some(self.loose.clone());
        }
        self.find_folder(root).cloned()
    }

    pub fn launcher_by_id(&self, id: &str) -> Option<&Launcher> {
        self.launchers.iter().find(|l| l.id == id)
    }

    fn ctx<'a>(&'a self, t: &Tab) -> TabCtx<'a> {
        let doc_missing = t.doc.as_ref().is_some_and(|d| {
            d.source.on_disk() && !d.path.as_deref().is_some_and(|p| std::path::Path::new(p).exists())
        });
        TabCtx {
            folder: self.folder_of(t),
            launcher: t.launcher_id.as_deref().and_then(|l| self.launcher_by_id(l)),
            doc_missing,
            now: self.now(),
        }
    }

    pub fn title(&self, t: &Tab) -> String {
        t.title(&self.ctx(t))
    }

    pub fn subtitle(&self, t: &Tab) -> String {
        t.subtitle(&self.ctx(t))
    }

    pub fn title_of(&self, id: &str) -> String {
        self.tab(id).map(|t| self.title(t)).unwrap_or_default()
    }

    pub fn panes(&self) -> Option<&PaneNode> {
        self.panes.as_ref()
    }

    pub fn panes_mut(&mut self) -> Option<&mut PaneNode> {
        self.mark(Areas::PANES);
        self.panes.as_mut()
    }

    pub fn set_panes(&mut self, tree: Option<PaneNode>, focus: Option<&str>) {
        self.panes = tree;
        self.focused = focus.map(str::to_owned);
        self.mark(Areas::PANES | Areas::SIDEBAR);
    }

    pub fn focused_pane_id(&self) -> Option<&str> {
        self.focused.as_deref()
    }

    pub fn order(&self) -> Vec<String> {
        layout::order(self.panes.as_ref())
    }

    pub fn set_installed_plugins(&mut self, ids: Vec<String>) {
        self.installed_plugins = ids;
        self.mark(Areas::SIDEBAR);
    }

    pub(crate) fn fresh_id(&self, make: impl Fn(u64) -> String, taken: impl Fn(&str) -> bool) -> String {
        let mut micros = ids::micros_now();
        loop {
            let id = make(micros);
            if !taken(&id) {
                return id;
            }
            micros += 1;
        }
    }

    pub(crate) fn sync_eager(&mut self) {
        let eager = self
            .tabs
            .iter()
            .any(|t| t.kind == mx_core::model::TabKind::Claude && !t.exited && t.resume_id().is_none());
        let was = self.eager.replace(eager);
        if was == Some(eager) || !self.timers.is_set(&crate::timers::TimerKey::Agents) {
            return;
        }
        let gap = if eager { chrono::TimeDelta::zero() } else { crate::timers::AGENTS_IDLE };
        self.timers.set(crate::timers::TimerKey::Agents, self.now() + gap);
    }
}
