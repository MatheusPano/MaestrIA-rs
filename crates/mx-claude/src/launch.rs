use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value, json};

use crate::mcp::{SERVER_NAME, tool_name, tool_names};
use crate::sh::sh_quote;

const HOOK_EVENTS: [(&str, bool); 12] = [
    ("SessionStart", false),
    ("SessionEnd", false),
    ("UserPromptSubmit", false),
    ("PreToolUse", true),
    ("PostToolUse", true),
    ("PostToolUseFailure", true),
    ("Notification", false),
    ("Stop", false),
    ("StopFailure", false),
    ("SubagentStop", false),
    ("PreCompact", false),
    ("PostCompact", false),
];

pub fn settings_for(
    port: u16,
    status_line_file: &Path,
    tab_id: &str,
    cwd: Option<&str>,
    home: Option<&str>,
) -> String {
    settings_with_tools(port, status_line_file, tab_id, cwd, home, &[])
}

pub fn settings_with_tools(
    port: u16,
    status_line_file: &Path,
    tab_id: &str,
    cwd: Option<&str>,
    home: Option<&str>,
    plugin_tools: &[String],
) -> String {
    let url = format!("http://127.0.0.1:{port}/hook/{tab_id}");
    let user = user_status_line(cwd, home);
    let user_command = user
        .as_ref()
        .and_then(|u| u.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let encoded = if user_command.is_empty() {
        String::new()
    } else {
        STANDARD.encode(user_command.as_bytes())
    };
    let status_url = format!("http://127.0.0.1:{port}/status/{tab_id}");

    let mut hooks = Map::new();
    for (event, tool) in HOOK_EVENTS {
        let mut group = Map::new();
        if tool {
            group.insert("matcher".into(), json!("*"));
        }
        group.insert(
            "hooks".into(),
            json!([{"type": "http", "url": url, "timeout": 5}]),
        );
        hooks.insert(event.into(), json!([group]));
    }

    let mut status_line = Map::new();
    status_line.insert("type".into(), json!("command"));
    status_line.insert(
        "command".into(),
        json!(format!(
            "sh {} {} {}",
            sh_quote(&status_line_file.to_string_lossy()),
            sh_quote(&status_url),
            sh_quote(&encoded)
        )),
    );
    if let Some(padding) = user.as_ref().and_then(|u| u.get("padding"))
        && !padding.is_null()
    {
        status_line.insert("padding".into(), padding.clone());
    }

    let mut allow: Vec<String> = tool_names().into();
    allow.extend(plugin_tools.iter().map(|t| tool_name(t)));
    json!({
        "permissions": {"allow": allow},
        "hooks": hooks,
        "statusLine": status_line,
    })
    .to_string()
}

pub fn mcp_config_for(port: u16, tab_id: &str) -> String {
    let mut servers = Map::new();
    servers.insert(
        SERVER_NAME.into(),
        json!({
            "type": "http",
            "url": format!("http://127.0.0.1:{port}/mcp/{tab_id}"),
            "alwaysLoad": true,
        }),
    );
    json!({ "mcpServers": servers }).to_string()
}

pub fn user_status_line(cwd: Option<&str>, home: Option<&str>) -> Option<Map<String, Value>> {
    let mut candidates = Vec::new();
    if let Some(cwd) = cwd.filter(|c| !c.is_empty()) {
        candidates.push(format!("{cwd}/.claude/settings.local.json"));
        candidates.push(format!("{cwd}/.claude/settings.json"));
    }
    if let Some(home) = home.filter(|h| !h.is_empty()) {
        candidates.push(format!("{home}/.claude/settings.json"));
    }
    candidates.into_iter().find_map(|path| {
        let text = std::fs::read_to_string(path).ok()?;
        let Ok(Value::Object(mut json)) = serde_json::from_str::<Value>(&text) else {
            return None;
        };
        match json.remove("statusLine") {
            Some(Value::Object(line))
                if line.get("type").and_then(Value::as_str) == Some("command")
                    && line.get("command").is_some_and(Value::is_string) =>
            {
                Some(line)
            }
            _ => None,
        }
    })
}

#[derive(Debug, Clone, Default)]
pub struct ClaudeLaunch {
    pub port: u16,
    pub mcp_config: String,
    pub settings: String,
    pub name: String,
    pub brief: Option<String>,
    pub project: Option<String>,
    pub resume_id: Option<String>,
    pub prompt: Option<String>,
}

impl ClaudeLaunch {
    fn brief(&self) -> Option<&str> {
        self.brief
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
    }

    fn prompt(&self) -> Option<&str> {
        self.prompt
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
    }

    pub fn command_line(&self) -> String {
        let mut parts = vec![
            "claude".to_owned(),
            format!("--mcp-config {}", sh_quote(&self.mcp_config)),
            format!("--name {}", sh_quote(&self.name)),
            format!("--settings {}", sh_quote(&self.settings)),
        ];
        if let Some(brief) = self.brief() {
            parts.push(format!("--append-system-prompt {}", sh_quote(brief)));
        }
        if let Some(resume) = &self.resume_id {
            parts.push(format!("--resume {}", sh_quote(resume)));
        }
        if let Some(prompt) = self.prompt() {
            parts.push(format!("-- {}", sh_quote(prompt)));
        }
        parts.join(" ")
    }

    pub fn display_line(&self) -> String {
        let port = self.port;
        let mut out = format!(
            "claude --mcp-config <tarefas :{port}> --name {} --settings <hooks :{port}>",
            sh_quote(&self.name)
        );
        if self.brief().is_some() {
            let project = self.project.as_deref().unwrap_or("");
            out.push_str(&format!(" --append-system-prompt <briefing de {project}>"));
        }
        if self.resume_id.is_some() {
            out.push_str(" --resume …");
        }
        if self.prompt().is_some() {
            out.push_str(" <prompt inicial>");
        }
        out
    }
}
