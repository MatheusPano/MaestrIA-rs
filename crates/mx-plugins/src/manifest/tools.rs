use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::{ManifestError, show};

pub const RESERVED_TOOLS: [&str; 2] = ["suggest_task", "dismiss_task"];

#[derive(Debug, Clone, PartialEq)]
pub struct PluginTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

static TOOL_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_]{0,47}$").unwrap());

const SHAPE: &str = "\"contributes.tools\" é uma lista: \
    [{\"name\": \"x_abrir\", \"description\": \"…\", \"inputSchema\": {\"type\": \"object\"}}]";

fn err(message: impl Into<String>) -> ManifestError {
    ManifestError::new(message)
}

pub(super) fn parse(raw: Option<&Value>, plugin_id: &str, has_main: bool) -> Result<Vec<PluginTool>, ManifestError> {
    let list = match raw {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(list)) => list,
        Some(_) => return Err(err(SHAPE)),
    };
    if !list.is_empty() && !has_main {
        return Err(err("\"contributes.tools\" precisa de \"main\": quem responde a ferramenta é o processo"));
    }
    let prefix = format!("{}_", plugin_id.rsplit('.').next().unwrap_or(plugin_id));
    let mut tools: Vec<PluginTool> = Vec::new();
    for item in list {
        let Value::Object(m) = item else { return Err(err(SHAPE)) };
        let name = m.get("name").map(show).unwrap_or_default();
        if !TOOL_NAME.is_match(&name) {
            return Err(err(format!(
                "nome de ferramenta inválido: \"{name}\" — minúsculas, dígitos e sublinhado, \
                 começando por letra, até 48 caracteres"
            )));
        }
        if RESERVED_TOOLS.contains(&name.as_str()) {
            return Err(err(format!("\"{name}\" é uma ferramenta da MaestrIA — escolha outro nome")));
        }
        if !name.starts_with(&prefix) {
            return Err(err(format!("a ferramenta \"{name}\" tem que começar com \"{prefix}\"")));
        }
        if tools.iter().any(|t| t.name == name) {
            return Err(err(format!("ferramenta repetida: \"{name}\"")));
        }
        let input_schema = match m.get("inputSchema") {
            Some(s @ Value::Object(o)) if o.get("type").and_then(Value::as_str) == Some("object") => s.clone(),
            _ => {
                return Err(err(format!(
                    "a ferramenta \"{name}\" precisa de \"inputSchema\" com \"type\": \"object\""
                )));
            }
        };
        let description = m.get("description").and_then(Value::as_str).unwrap_or("").trim().to_owned();
        tools.push(PluginTool { name, description, input_schema });
    }
    Ok(tools)
}
