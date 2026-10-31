mod commands;
mod contributes;
mod settings;
mod tools;

use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

use mx_core::json::Obj;
use regex::Regex;
use serde_json::Value;

pub use commands::{CommandTarget, PluginCommand};
pub use settings::{PluginSetting, SettingOption, SettingType};
pub use tools::{PluginTool, RESERVED_TOOLS};

use crate::permission::Permission;
use crate::{MANIFEST_NAME, PLUGIN_API};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestError(pub String);

impl ManifestError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ManifestError {}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub main: Option<Vec<String>>,
    pub activation: Vec<String>,
    pub permissions: Vec<Permission>,
    pub commands: Vec<PluginCommand>,
    pub themes: Vec<String>,
    pub api: i64,
    pub warnings: Vec<String>,
    pub icon: Option<String>,
    pub settings: Vec<PluginSetting>,
    pub sidebar: bool,
    pub screen: bool,
    pub screen_home: Option<String>,
    pub suggestions: Option<String>,
    pub tools: Vec<PluginTool>,
}

static PLUGIN_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]*$").unwrap());

pub(crate) fn show(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        other => other.to_string(),
    }
}

fn need(j: &Obj, key: &str) -> Result<String, ManifestError> {
    match j.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_owned()),
        _ => Err(ManifestError::new(format!("\"{key}\" é obrigatório"))),
    }
}

impl PluginManifest {
    pub fn has_process(&self) -> bool {
        self.main.as_ref().is_some_and(|m| !m.is_empty())
    }

    pub fn kinds(&self) -> std::collections::BTreeSet<crate::catalog::PluginKind> {
        use crate::catalog::PluginKind;
        let mut out = std::collections::BTreeSet::new();
        if !self.themes.is_empty() {
            out.insert(PluginKind::Theme);
        }
        if self.has_process()
            || !self.commands.is_empty()
            || !self.settings.is_empty()
            || self.sidebar
            || self.screen
            || self.suggestions.is_some()
            || !self.tools.is_empty()
        {
            out.insert(PluginKind::Feature);
        }
        out
    }

    pub fn activates_on(&self, event: &str) -> bool {
        if !self.has_process() {
            return false;
        }
        if self.activation.is_empty() {
            return event == "onStartup";
        }
        self.activation.iter().any(|e| e == "*" || e == event)
    }

    pub fn allows(&self, p: Permission) -> bool {
        self.permissions.contains(&p)
    }

    pub fn parse(j: &Obj) -> Result<PluginManifest, ManifestError> {
        Self::parse_for(j, cfg!(target_os = "linux"))
    }

    pub fn parse_for(j: &Obj, linux: bool) -> Result<PluginManifest, ManifestError> {
        let mut warnings = Vec::new();
        let id = need(j, "id")?;
        if !PLUGIN_ID.is_match(&id) {
            return Err(ManifestError::new(format!(
                "\"id\" só pode ter minúsculas, dígitos, ponto, hífen e sublinhado: \"{id}\""
            )));
        }
        let api = j.get("maestria").and_then(Value::as_f64).map_or(PLUGIN_API, |n| n.trunc() as i64);
        if api > PLUGIN_API {
            return Err(ManifestError::new(format!(
                "o plugin pede a versão {api} da API e esta MaestrIA só conhece até a {PLUGIN_API}"
            )));
        }
        let main = parse_main(j.get("main"))?;
        let permissions = parse_permissions(j.get("permissions"))?;
        let mut activation: Vec<String> = Vec::new();
        for e in j.get("activationEvents").and_then(Value::as_array).into_iter().flatten() {
            if let Some(s) = e.as_str().filter(|s| !activation.iter().any(|a| a == s)) {
                activation.push(s.to_owned());
            }
        }
        let empty = Obj::new();
        let contributes = j.get("contributes").and_then(Value::as_object).unwrap_or(&empty);
        let commands =
            commands::parse_commands(&id, contributes.get("commands"), main.is_some(), linux, &mut warnings)?;
        let c = contributes::parse(contributes, &id, &commands, main.is_some(), &mut warnings)?;
        let name = match j.get("name") {
            Some(Value::String(n)) if !n.trim().is_empty() => n.trim().to_owned(),
            _ => id.clone(),
        };
        let author = match j.get("author") {
            Some(Value::String(a)) => a.clone(),
            Some(Value::Object(m)) => m.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
            _ => String::new(),
        };
        Ok(PluginManifest {
            version: need(j, "version")?,
            name,
            description: j.get("description").and_then(Value::as_str).unwrap_or("").to_owned(),
            author,
            main,
            activation,
            permissions,
            commands,
            themes: c.themes,
            api,
            warnings,
            icon: j.get("icon").and_then(Value::as_str).map(str::to_owned),
            settings: c.settings,
            sidebar: c.sidebar,
            screen: c.screen,
            screen_home: c.screen_home,
            suggestions: c.suggestions,
            tools: c.tools,
            id,
        })
    }

    pub fn read(dir: &Path) -> Result<PluginManifest, ManifestError> {
        let file = dir.join(MANIFEST_NAME);
        if !file.is_file() {
            return Err(ManifestError::new(format!("não tem {MANIFEST_NAME}")));
        }
        let text = std::fs::read(&file).map_err(|e| ManifestError::new(e.to_string()))?;
        let json: Value = serde_json::from_slice(&text)
            .map_err(|e| ManifestError::new(format!("{MANIFEST_NAME} não é um json válido: {e}")))?;
        let Value::Object(j) = json else {
            return Err(ManifestError::new(format!("{MANIFEST_NAME} precisa ser um objeto")));
        };
        Self::parse(&j)
    }
}

fn parse_main(raw: Option<&Value>) -> Result<Option<Vec<String>>, ManifestError> {
    match raw {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(cmd)) if !cmd.trim().is_empty() => Ok(Some(vec![cmd.trim().to_owned()])),
        Some(Value::Array(list)) if !list.is_empty() && list.iter().all(Value::is_string) => {
            Ok(Some(list.iter().filter_map(Value::as_str).map(str::to_owned).collect()))
        }
        _ => Err(ManifestError::new("\"main\" é uma lista: [\"node\", \"main.js\"]")),
    }
}

fn parse_permissions(raw: Option<&Value>) -> Result<Vec<Permission>, ManifestError> {
    let mut out = Vec::new();
    for p in raw.and_then(Value::as_array).into_iter().flatten() {
        let Some(perm) = p.as_str().and_then(Permission::by_id) else {
            let known: Vec<&str> = Permission::ALL.iter().map(|p| p.id()).collect();
            return Err(ManifestError::new(format!(
                "permissão desconhecida: \"{}\" — as que existem são {}",
                show(p),
                known.join(", ")
            )));
        };
        if !out.contains(&perm) {
            out.push(perm);
        }
    }
    Ok(out)
}
