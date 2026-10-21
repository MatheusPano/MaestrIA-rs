use std::collections::HashMap;

use mx_core::model::{SuggestedTask, SuggestionChoice};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionOrigin {
    pub tab_id: String,
    pub cwd: String,
    pub root: String,
    pub folder: String,
    pub is_repo: bool,
    pub branch: String,
}

pub fn choice_name(choice: SuggestionChoice) -> &'static str {
    match choice {
        SuggestionChoice::Worktree => "worktree",
        SuggestionChoice::Local => "local",
        SuggestionChoice::Here => "here",
        SuggestionChoice::Dismiss => "dismiss",
    }
}

pub fn start_params(task: &SuggestedTask, choice: SuggestionChoice, origin: &SuggestionOrigin) -> Value {
    let mut suggestion = Map::new();
    suggestion.insert("id".into(), Value::from(task.id.clone()));
    suggestion.insert("title".into(), Value::from(task.title.clone()));
    suggestion.insert("tldr".into(), Value::from(task.tldr.clone()));
    suggestion.insert("prompt".into(), Value::from(task.prompt.clone()));
    let mut o = Map::new();
    o.insert("tabId".into(), Value::from(origin.tab_id.clone()));
    o.insert("cwd".into(), Value::from(origin.cwd.clone()));
    o.insert("root".into(), Value::from(origin.root.clone()));
    o.insert("folder".into(), Value::from(origin.folder.clone()));
    o.insert("isRepo".into(), Value::from(origin.is_repo));
    o.insert("branch".into(), Value::from(origin.branch.clone()));
    let mut p = Map::new();
    p.insert("suggestion".into(), Value::Object(suggestion));
    p.insert("choice".into(), Value::from(choice_name(choice)));
    p.insert("origin".into(), Value::Object(o));
    Value::Object(p)
}

pub fn suggestion_prompt(intro: &str, task_prompt: &str, on_branch: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !intro.trim().is_empty() {
        parts.push(intro.trim().to_owned());
    }
    parts.push(task_prompt.to_owned());
    if let Some(b) = on_branch {
        parts.push(format!("Você está na branch `{b}`. Leia o código antes de propor o plano."));
    }
    parts.join("\n\n")
}

pub fn worktree_branch(branch: Option<&str>, reference: &str) -> String {
    branch.map_or_else(|| format!("feature/{reference}"), str::to_owned)
}

pub fn worktree_dir(dir: Option<&str>, reference: &str) -> String {
    dir.unwrap_or(reference).to_owned()
}

pub fn gone_banner(reference: &str) -> String {
    format!("a tarefa {reference} foi criada, mas a sugestão já não estava mais no cartão")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracking {
    pub plugin_id: String,
    pub tab_id: String,
    pub choice: SuggestionChoice,
}

#[derive(Debug, Clone, Default)]
pub struct SuggestionTracker {
    pending: HashMap<String, Tracking>,
}

impl SuggestionTracker {
    pub fn track(&mut self, suggestion_id: &str, tracking: Tracking) {
        self.pending.insert(suggestion_id.to_owned(), tracking);
    }

    pub fn forget(&mut self, suggestion_id: &str) {
        self.pending.remove(suggestion_id);
    }

    pub fn take(&mut self, plugin_id: &str, suggestion_id: &str) -> Option<Tracking> {
        if self.pending.get(suggestion_id)?.plugin_id != plugin_id {
            return None;
        }
        self.pending.remove(suggestion_id)
    }

    pub fn is_tracked(&self, suggestion_id: &str) -> bool {
        self.pending.contains_key(suggestion_id)
    }
}
