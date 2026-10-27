use std::sync::LazyLock;

use chrono::Timelike;
use regex::Regex;
use serde_json::{Value, json};

use crate::json::Obj;
use crate::model::{
    ClaudeStatus, ContextUsage, Doc, Folder, FollowUp, HookState, Launcher, PluginViewRef, Setup,
    Tint, worktree::DETACHED,
};
use crate::text::{Time, last_segment, short_ago};

static BRANCH_LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"((?-u:[A-Za-z])+[#-]?(?-u:\d)+)").expect("valid regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TabKind {
    Shell,
    Claude,
    Reader,
    Setup,
    Plugin,
}

impl TabKind {
    pub const ALL: [TabKind; 5] = [
        Self::Shell,
        Self::Claude,
        Self::Reader,
        Self::Setup,
        Self::Plugin,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Claude => "claude",
            Self::Reader => "reader",
            Self::Setup => "setup",
            Self::Plugin => "plugin",
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
}

pub struct TabCtx<'a> {
    pub folder: &'a Folder,
    pub launcher: Option<&'a Launcher>,
    pub doc_missing: bool,
    pub now: Time,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub id: String,
    pub folder_root: String,
    pub loose: bool,
    pub kind: TabKind,
    pub cwd: String,
    pub branch: String,
    pub custom_label: Option<String>,
    pub tint: Option<Tint>,
    pub hooks: HookState,
    pub context: Option<ContextUsage>,
    pub doc: Option<Doc>,
    pub setup: Option<Setup>,
    pub view: Option<PluginViewRef>,
    pub owner: Option<String>,
    pub owner_tag: Option<String>,
    pub embedded: bool,
    pub launcher_id: Option<String>,
    pub feature_or_hotfix_id: Option<String>,
    pub born_of: Option<String>,
    pub armed: bool,
    pub follow_ups: Vec<FollowUp>,
    pub session_id: Option<String>,
    pub agent_name: Option<String>,
    pub hotkey: Option<String>,
    pub started_at: Time,
    pub zoom: i32,
    pub group_id: Option<String>,
    pub done: bool,
    pub pinned: bool,
    pub on_sessions: bool,
    pub hibernated: bool,
    pub rested_at: Option<Time>,
    pub unseen: bool,
    pub exited: bool,
    pub exit_code: Option<i32>,
}

impl Tab {
    pub fn new(
        id: impl Into<String>,
        folder: &Folder,
        kind: TabKind,
        cwd: impl Into<String>,
        now: Time,
    ) -> Tab {
        Tab {
            id: id.into(),
            folder_root: folder.root.clone(),
            loose: folder.is_loose,
            kind,
            cwd: cwd.into(),
            branch: String::new(),
            custom_label: None,
            tint: None,
            hooks: HookState::new(),
            context: None,
            doc: None,
            setup: None,
            view: None,
            owner: None,
            owner_tag: None,
            embedded: false,
            launcher_id: None,
            feature_or_hotfix_id: None,
            born_of: None,
            armed: false,
            follow_ups: Vec::new(),
            session_id: None,
            agent_name: None,
            hotkey: None,
            started_at: now,
            zoom: 0,
            group_id: None,
            done: false,
            pinned: false,
            on_sessions: false,
            hibernated: false,
            rested_at: None,
            unseen: false,
            exited: false,
            exit_code: None,
        }
    }

    pub fn is_reader(&self) -> bool {
        self.kind == TabKind::Reader && self.doc.is_some()
    }

    pub fn is_setup(&self) -> bool {
        self.kind == TabKind::Setup && self.setup.is_some()
    }

    pub fn is_plugin_view(&self) -> bool {
        self.kind == TabKind::Plugin && self.view.is_some()
    }

    pub fn is_passive(&self) -> bool {
        self.is_reader() || self.is_setup() || self.is_plugin_view()
    }

    pub fn resume_id(&self) -> Option<&str> {
        self.session_id
            .as_deref()
            .or(self.hooks.session_id.as_deref())
    }

    pub fn resumable(&self) -> bool {
        self.kind == TabKind::Claude && self.resume_id().is_some()
    }

    pub fn can_hibernate(&self) -> bool {
        self.kind == TabKind::Claude
            && !self.exited
            && !self.hibernated
            && self.resume_id().is_some()
    }

    pub fn status(&self) -> ClaudeStatus {
        match self.kind {
            TabKind::Reader | TabKind::Setup | TabKind::Plugin => ClaudeStatus::Unknown,
            TabKind::Shell if self.exited => ClaudeStatus::Ended,
            TabKind::Shell => ClaudeStatus::Unknown,
            TabKind::Claude => self.hooks.status,
        }
    }

    pub fn rested(&self) -> bool {
        self.rested_at.is_some() && !self.exited && self.status().at_rest()
    }

    pub fn rested_ago(&self, now: &Time) -> Option<String> {
        if !self.rested() {
            return None;
        }
        self.rested_at.as_ref().map(|at| short_ago(at, now))
    }

    pub fn title(&self, ctx: &TabCtx) -> String {
        if let Some(label) = self.custom_label.as_deref().filter(|l| !l.is_empty()) {
            return label.to_owned();
        }
        if let Some(doc) = &self.doc {
            return doc.title.clone();
        }
        if let Some(setup) = &self.setup {
            return format!("Claude · {}", setup.name());
        }
        if let Some(view) = &self.view {
            return view.title.clone();
        }
        if let Some(l) = ctx.launcher {
            return l.name.clone();
        }
        if ctx.folder.is_loose {
            return if self.cwd == ctx.folder.root {
                "~".into()
            } else {
                last_segment(&self.cwd).to_owned()
            };
        }
        if self.cwd == ctx.folder.root {
            return ctx.folder.name.clone();
        }
        if !self.branch.is_empty() && self.branch != DETACHED {
            if let Some(m) = BRANCH_LABEL.find(&self.branch) {
                return m.as_str().to_owned();
            }
            return last_segment(&self.branch).to_owned();
        }
        last_segment(&self.cwd).to_owned()
    }

    pub fn subtitle(&self, ctx: &TabCtx) -> String {
        if let Some(doc) = &self.doc {
            if ctx.doc_missing {
                return format!("{} · o arquivo não está mais lá", doc.source.label());
            }
            let when = format!("{:02}:{:02}", doc.at.hour(), doc.at.minute());
            let mut parts = vec![doc.source.label().to_owned()];
            if let Some(from) = doc.origin.as_deref().filter(|o| !o.is_empty())
                && !self.title(ctx).contains(from)
            {
                parts.push(format!("de {from}"));
            }
            parts.push(when);
            return parts.join(" · ");
        }
        if let Some(setup) = &self.setup {
            return setup
                .selected
                .clone()
                .unwrap_or_else(|| "Configuração do Claude".into());
        }
        if let Some(view) = &self.view {
            return format!("Plugin · {}", view.plugin_name);
        }
        if self.hibernated {
            return "Hibernada · clique pra retomar".into();
        }
        if self.exited {
            return match self.exit_code {
                Some(c) => format!("Processo saiu ({c})"),
                None => "Processo saiu (?)".into(),
            };
        }
        if let Some(l) = ctx.launcher {
            return l.command.clone();
        }
        if self.kind == TabKind::Shell {
            return "Shell".into();
        }
        self.hooks.subtitle(&ctx.now)
    }

    pub fn recipe(&self) -> Obj {
        let mut m = Obj::new();
        m.insert("folderRoot".into(), json!(self.folder_root));
        if self.loose {
            m.insert("loose".into(), json!(true));
        }
        if let Some(f) = &self.feature_or_hotfix_id {
            m.insert("featureOrHotfixId".into(), json!(f));
        }
        m.insert("kind".into(), json!(self.kind.name()));
        m.insert("cwd".into(), json!(self.cwd));
        if let Some(l) = &self.launcher_id {
            m.insert("launcher".into(), json!(l));
        }
        if let Some(label) = &self.custom_label {
            m.insert("label".into(), json!(label));
        }
        if let Some(t) = self.tint {
            m.insert("tint".into(), json!(t.name()));
        }
        if self.resumable()
            && let Some(id) = self.resume_id()
        {
            m.insert("sessionId".into(), json!(id));
        }
        if let Some(doc) = &self.doc {
            m.insert("doc".into(), doc.to_json());
        }
        if let Some(setup) = &self.setup {
            m.insert("setup".into(), setup.to_json());
        }
        if self.zoom != 0 {
            m.insert("zoom".into(), json!(self.zoom));
        }
        m
    }

    pub fn to_json(&self) -> Value {
        let mut m = self.recipe();
        if let Some(g) = &self.group_id {
            m.insert("group".into(), json!(g));
        }
        if !self.hooks.touched.is_empty() {
            m.insert("touched".into(), json!(self.hooks.touched));
        }
        if !self.hooks.plans.is_empty() {
            let plans = self.hooks.plans.iter().map(|p| p.to_json()).collect();
            m.insert("plans".into(), Value::Array(plans));
        }
        if !self.hooks.suggestions.is_empty() {
            let tasks = self.hooks.suggestions.iter().map(|t| t.to_json()).collect();
            m.insert("suggestions".into(), Value::Array(tasks));
        }
        if self.done {
            m.insert("done".into(), json!(true));
        }
        if self.hibernated {
            m.insert("hibernated".into(), json!(true));
        }
        if self.pinned {
            m.insert("pinned".into(), json!(true));
        }
        if let Some(k) = &self.hotkey {
            m.insert("hotkey".into(), json!(k));
        }
        Value::Object(m)
    }
}
