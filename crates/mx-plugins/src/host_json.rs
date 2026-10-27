use mx_core::model::{ClaudeStatus, ContextUsage, FeatureOrHotfixKind, TabKind, WorktreeInfo};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct SessionActivity {
    pub prompts: i64,
    pub tools: i64,
    pub touched: Vec<String>,
    pub last_prompt: Option<String>,
    pub last_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionJson {
    pub id: String,
    pub kind: TabKind,
    pub title: String,
    pub cwd: String,
    pub folder: String,
    pub branch: String,
    pub status: ClaudeStatus,
    pub done: bool,
    pub session_id: Option<String>,
    pub hibernated: bool,
    pub exited: bool,
    pub on_screen: bool,
    pub focused: bool,
    pub started_at: String,
    pub feature_or_hotfix: Option<(String, FeatureOrHotfixKind)>,
    pub workspaces: Vec<String>,
    pub launcher: Option<String>,
    pub context: Option<ContextUsage>,
    pub owner: Option<String>,
    pub owner_tag: Option<String>,
    pub embedded: bool,
    pub activity: SessionActivity,
}

pub fn context_json(c: Option<&ContextUsage>) -> Value {
    c.map_or(Value::Null, |c| {
        let mut m = Map::new();
        m.insert("percent".into(), Value::from(c.percent));
        m.insert("tokens".into(), Value::from(c.tokens));
        m.insert("window".into(), Value::from(c.window));
        Value::Object(m)
    })
}

fn opt(v: &Option<String>) -> Value {
    v.clone().map_or(Value::Null, Value::from)
}

fn strings(v: &[String]) -> Value {
    v.iter().cloned().map(Value::from).collect()
}

impl SessionJson {
    pub fn to_json(&self, caller_id: &str, caller_has_hooks: bool) -> Value {
        let status = if self.kind == TabKind::Claude || self.kind == TabKind::Shell {
            self.status
        } else {
            ClaudeStatus::Unknown
        };
        let feature = self.feature_or_hotfix.as_ref();
        let mut m = Map::new();
        m.insert("id".into(), Value::from(self.id.clone()));
        m.insert("kind".into(), Value::from(self.kind.name()));
        m.insert("title".into(), Value::from(self.title.clone()));
        m.insert("cwd".into(), Value::from(self.cwd.clone()));
        m.insert("folder".into(), Value::from(self.folder.clone()));
        m.insert("branch".into(), Value::from(self.branch.clone()));
        m.insert("status".into(), Value::from(status.name()));
        m.insert("statusLabel".into(), Value::from(status.label()));
        m.insert("needsYou".into(), Value::from(!self.done && status.needs_human()));
        m.insert("sessionId".into(), opt(&self.session_id));
        m.insert("done".into(), Value::from(self.done));
        m.insert("hibernated".into(), Value::from(self.hibernated));
        m.insert("exited".into(), Value::from(self.exited));
        m.insert("onScreen".into(), Value::from(self.on_screen));
        m.insert("focused".into(), Value::from(self.focused));
        m.insert("startedAt".into(), Value::from(self.started_at.clone()));
        let name = feature.map_or(Value::Null, |(n, _)| Value::from(n.clone()));
        m.insert("project".into(), name.clone());
        m.insert("featureOrHotfix".into(), name);
        m.insert(
            "featureOrHotfixKind".into(),
            feature.map_or(Value::Null, |(_, k)| Value::from(k.name())),
        );
        m.insert("workspaces".into(), strings(&self.workspaces));
        m.insert("launcher".into(), opt(&self.launcher));
        m.insert("context".into(), context_json(self.context.as_ref()));
        m.insert("owner".into(), opt(&self.owner));
        m.insert("embedded".into(), Value::from(self.embedded));
        if self.owner.as_deref() == Some(caller_id) {
            m.insert("tag".into(), opt(&self.owner_tag));
        }
        if caller_has_hooks {
            let a = &self.activity;
            let mut act = Map::new();
            act.insert("prompts".into(), Value::from(a.prompts));
            act.insert("tools".into(), Value::from(a.tools));
            act.insert("touched".into(), strings(&a.touched));
            act.insert("lastPrompt".into(), opt(&a.last_prompt));
            act.insert("lastMessage".into(), opt(&a.last_message));
            m.insert("activity".into(), Value::Object(act));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FolderJson {
    pub name: String,
    pub root: String,
    pub is_repo: bool,
    pub branch: String,
    pub worktrees: Vec<WorktreeInfo>,
    pub workspaces: Vec<String>,
}

impl FolderJson {
    pub fn to_json(&self) -> Value {
        let worktrees: Vec<Value> = self
            .worktrees
            .iter()
            .map(|w| {
                let mut m = Map::new();
                m.insert("path".into(), Value::from(w.path.clone()));
                m.insert("branch".into(), Value::from(w.branch.clone()));
                m.insert("isMain".into(), Value::from(w.is_main));
                m.insert("label".into(), Value::from(w.short_label()));
                m.insert("prunable".into(), Value::from(w.prunable));
                Value::Object(m)
            })
            .collect();
        let mut m = Map::new();
        m.insert("name".into(), Value::from(self.name.clone()));
        m.insert("root".into(), Value::from(self.root.clone()));
        m.insert("isRepo".into(), Value::from(self.is_repo));
        m.insert("branch".into(), Value::from(self.branch.clone()));
        m.insert("worktrees".into(), Value::Array(worktrees));
        m.insert("workspaces".into(), strings(&self.workspaces));
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureJson {
    pub id: String,
    pub name: String,
    pub kind: FeatureOrHotfixKind,
    pub folder: String,
    pub brief: String,
}

impl FeatureJson {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), Value::from(self.id.clone()));
        m.insert("name".into(), Value::from(self.name.clone()));
        m.insert("kind".into(), Value::from(self.kind.name()));
        m.insert("folder".into(), Value::from(self.folder.clone()));
        m.insert("brief".into(), Value::from(self.brief.clone()));
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceJson {
    pub id: String,
    pub name: String,
    pub folders: Vec<String>,
    pub code_workspace_path: Option<String>,
}

impl WorkspaceJson {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), Value::from(self.id.clone()));
        m.insert("name".into(), Value::from(self.name.clone()));
        m.insert("folders".into(), strings(&self.folders));
        if let Some(p) = &self.code_workspace_path {
            m.insert("codeWorkspacePath".into(), Value::from(p.clone()));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatJson {
    pub session_id: String,
    pub title: String,
    pub cwd: String,
    pub folder: String,
    pub at: String,
    pub size: i64,
}

impl ChatJson {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("sessionId".into(), Value::from(self.session_id.clone()));
        m.insert("title".into(), Value::from(self.title.clone()));
        m.insert("cwd".into(), Value::from(self.cwd.clone()));
        m.insert("folder".into(), Value::from(self.folder.clone()));
        m.insert("at".into(), Value::from(self.at.clone()));
        m.insert("size".into(), Value::from(self.size));
        Value::Object(m)
    }
}
