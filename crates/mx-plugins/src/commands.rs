use std::sync::LazyLock;

use mx_core::json::Obj;
use regex::{Captures, Regex};
use serde_json::Value;

use crate::manifest::{CommandTarget, PluginCommand};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandValues {
    pub cwd: String,
    pub folder: String,
    pub session_id: String,
    pub title: String,
    pub plugin_dir: String,
}

impl CommandValues {
    pub fn get(&self, name: &str) -> Option<&str> {
        Some(match name {
            "cwd" => &self.cwd,
            "folder" => &self.folder,
            "sessionId" => &self.session_id,
            "title" => &self.title,
            "pluginDir" => &self.plugin_dir,
            _ => return None,
        })
    }

    pub fn context(&self, tab_id: Option<&str>) -> Obj {
        let mut m = Obj::new();
        for key in ["cwd", "folder", "sessionId", "title", "pluginDir"] {
            m.insert(key.into(), Value::from(self.get(key).unwrap_or_default()));
        }
        if let Some(t) = tab_id {
            m.insert("tabId".into(), Value::from(t));
        }
        m
    }
}

pub fn sh_quote(v: &str) -> String {
    format!("'{}'", v.replace('\'', "'\\''"))
}

static VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\{([A-Za-z0-9_]+)\}").unwrap());

pub fn expand(template: &str, lookup: impl Fn(&str) -> Option<String>, quote: bool) -> String {
    VAR.replace_all(template, |c: &Captures| match lookup(&c[1]) {
        None => c[0].to_owned(),
        Some(v) if quote => sh_quote(&v),
        Some(v) => v,
    })
    .into_owned()
}

pub fn expand_values(template: &str, values: &CommandValues, quote: bool) -> String {
    expand(template, |k| values.get(k).map(str::to_owned), quote)
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandRun {
    Terminal { command: String },
    Background { command: String },
    Session { text: String },
    Plugin { context: Obj },
}

pub fn plan(command: &PluginCommand, values: &CommandValues, tab_id: Option<&str>) -> CommandRun {
    let run = || expand_values(command.run.as_deref().unwrap_or_default(), values, true);
    match command.target {
        CommandTarget::Terminal => CommandRun::Terminal { command: run() },
        CommandTarget::Background => CommandRun::Background { command: run() },
        CommandTarget::Session => {
            CommandRun::Session { text: expand_values(command.send.as_deref().unwrap_or_default(), values, false) }
        }
        CommandTarget::Plugin => CommandRun::Plugin { context: values.context(tab_id) },
    }
}

fn last_line(text: &str) -> Option<&str> {
    text.split('\n').rfind(|l| !l.trim().is_empty())
}

pub fn background_banner(title: &str, code: i32, stdout: &str, stderr: &str) -> (String, bool) {
    if code == 0 {
        return (format!("{title}: {}", last_line(stdout).unwrap_or("feito")), false);
    }
    let said = last_line(if stderr.is_empty() { stdout } else { stderr });
    let tail = said.map_or_else(String::new, |s| format!(": {s}"));
    (format!("{title} falhou (código {code}){tail}"), true)
}

pub fn no_session_banner(title: &str) -> String {
    format!("{title}: precisa de uma sessão do Claude rodando em foco")
}

pub fn failure_banner(title: &str, message: &str) -> String {
    format!("{title}: {message}")
}
