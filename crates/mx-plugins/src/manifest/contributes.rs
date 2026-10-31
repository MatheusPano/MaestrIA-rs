use mx_core::json::Obj;
use serde_json::Value;

use super::{ManifestError, PluginCommand, PluginSetting, PluginTool};

pub(super) struct Contributes {
    pub themes: Vec<String>,
    pub settings: Vec<PluginSetting>,
    pub sidebar: bool,
    pub screen: bool,
    pub screen_home: Option<String>,
    pub suggestions: Option<String>,
    pub tools: Vec<PluginTool>,
}

const KNOWN: [&str; 7] = ["commands", "themes", "settings", "sidebar", "screen", "suggestions", "tools"];

fn err(message: impl Into<String>) -> ManifestError {
    ManifestError::new(message)
}

pub(super) fn parse(
    c: &Obj,
    plugin_id: &str,
    commands: &[PluginCommand],
    has_main: bool,
    warnings: &mut Vec<String>,
) -> Result<Contributes, ManifestError> {
    let mut themes = Vec::new();
    for t in c.get("themes").and_then(Value::as_array).into_iter().flatten() {
        let path = match t {
            Value::String(p) => Some(p.clone()),
            Value::Object(m) => m.get("path").and_then(Value::as_str).map(str::to_owned),
            _ => None,
        };
        themes.push(path.ok_or_else(|| err("cada tema é {\"path\": \"themes/x.json\"}"))?);
    }

    let mut settings: Vec<PluginSetting> = Vec::new();
    for raw in c.get("settings").and_then(Value::as_array).into_iter().flatten() {
        let Value::Object(m) = raw else { return Err(err("cada configuração é um objeto")) };
        let setting = PluginSetting::parse(m)?;
        if settings.iter().any(|o| o.id == setting.id) {
            return Err(err(format!("configuração repetida: \"{}\"", setting.id)));
        }
        settings.push(setting);
    }

    let sidebar = match c.get("sidebar") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(err("\"contributes.sidebar\" é true ou false")),
    };
    if sidebar && !has_main {
        return Err(err("\"contributes.sidebar\" precisa de \"main\" pra desenhar a aba"));
    }

    let (screen, screen_home) = match c.get("screen") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => (false, None),
        Some(Value::Bool(true)) => (true, None),
        Some(Value::Object(m)) if m.get("home").is_some_and(Value::is_string) => {
            let home = m.get("home").and_then(Value::as_str).unwrap_or_default();
            if !commands.iter().any(|c| c.id == home) {
                return Err(err(format!(
                    "\"contributes.screen.home\" aponta pro comando \"{home}\", que o manifesto não declara"
                )));
            }
            (true, Some(home.to_owned()))
        }
        Some(_) => {
            return Err(err("\"contributes.screen\" é true ou um objeto com \"home\": {\"home\": \"quadro\"}"));
        }
    };
    if screen && !has_main {
        return Err(err("\"contributes.screen\" precisa de \"main\" pra abrir as janelas"));
    }

    let suggestions = match c.get("suggestions") {
        None | Some(Value::Null) => None,
        Some(Value::Object(m))
            if m.get("label").and_then(Value::as_str).is_some_and(|l| !l.trim().is_empty()) =>
        {
            if !has_main {
                return Err(err(
                    "\"contributes.suggestions\" precisa de \"main\": quem recebe a sugestão é o processo",
                ));
            }
            m.get("label").and_then(Value::as_str).map(|l| l.trim().to_owned())
        }
        Some(_) => {
            return Err(err(
                "\"contributes.suggestions\" é um objeto com \"label\": {\"label\": \"criar a task no …\"}",
            ));
        }
    };

    let tools = super::tools::parse(c.get("tools"), plugin_id, has_main)?;

    for k in c.keys() {
        if !KNOWN.contains(&k.as_str()) {
            warnings.push(format!("\"contributes.{k}\" não existe nesta versão — ignorado"));
        }
    }
    Ok(Contributes { themes, settings, sidebar, screen, screen_home, suggestions, tools })
}
