use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::{ManifestError, show};
use crate::chord::Chord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTarget {
    Plugin,
    Terminal,
    Background,
    Session,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginCommand {
    pub plugin_id: String,
    pub id: String,
    pub title: String,
    pub target: CommandTarget,
    pub run: Option<String>,
    pub send: Option<String>,
    pub key: Option<Chord>,
    pub icon: Option<String>,
    pub sidebar: bool,
}

impl PluginCommand {
    pub fn full_id(&self) -> String {
        format!("{}/{}", self.plugin_id, self.id)
    }

    pub fn short_title(&self, plugin_name: &str) -> &str {
        match self.title.split_once(": ") {
            Some((prefix, rest)) if prefix.to_lowercase() == plugin_name.to_lowercase() => rest,
            _ => &self.title,
        }
    }
}

static COMMAND_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._-]+$").unwrap());

pub(super) fn parse_commands(
    plugin_id: &str,
    raw: Option<&Value>,
    has_main: bool,
    linux: bool,
    warnings: &mut Vec<String>,
) -> Result<Vec<PluginCommand>, ManifestError> {
    let mut commands: Vec<PluginCommand> = Vec::new();
    let Some(Value::Array(items)) = raw else { return Ok(commands) };
    for raw in items {
        let Value::Object(c) = raw else {
            return Err(ManifestError::new("cada comando é um objeto"));
        };
        let cid = match c.get("id") {
            Some(Value::String(s)) if COMMAND_ID.is_match(s) => s.clone(),
            other => {
                let shown = other.map_or_else(|| "null".to_owned(), Value::to_string);
                return Err(ManifestError::new(format!("comando com \"id\" inválido: {shown}")));
            }
        };
        let title = match c.get("title") {
            Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_owned(),
            _ => return Err(ManifestError::new(format!("o comando \"{cid}\" precisa de \"title\""))),
        };
        if commands.iter().any(|o| o.id == cid) {
            return Err(ManifestError::new(format!("comando repetido: \"{cid}\"")));
        }
        let run = c.get("run").and_then(Value::as_str).map(str::to_owned);
        let send = c.get("send").and_then(Value::as_str).map(str::to_owned);
        if run.is_some() && send.is_some() {
            return Err(ManifestError::new(format!("o comando \"{cid}\" tem \"run\" e \"send\" — escolha um")));
        }
        let target = if send.is_some() {
            CommandTarget::Session
        } else if run.is_some() {
            match c.get("in") {
                None | Some(Value::Null) => CommandTarget::Terminal,
                Some(Value::String(s)) if s == "terminal" => CommandTarget::Terminal,
                Some(Value::String(s)) if s == "background" => CommandTarget::Background,
                Some(other) => {
                    return Err(ManifestError::new(format!(
                        "o comando \"{cid}\" tem \"in\": \"{}\" — vale \"terminal\" ou \"background\"",
                        show(other)
                    )));
                }
            }
        } else {
            if !has_main {
                return Err(ManifestError::new(format!(
                    "o comando \"{cid}\" não tem \"run\" nem \"send\", e o plugin não tem \"main\" pra atendê-lo"
                )));
            }
            CommandTarget::Plugin
        };
        let mut key = None;
        if let Some(Value::String(k)) = c.get("key") {
            match Chord::parse(k).map(|ch| ch.for_host(linux)) {
                None => warnings.push(format!(
                    "o comando \"{cid}\" pede a tecla \"{k}\", que não dá pra ler — ficou sem"
                )),
                Some(chord) => match chord.rejection(linux) {
                    Some(why) => warnings.push(format!(
                        "o comando \"{cid}\" pede {}: {why} — ficou sem",
                        chord.label(linux)
                    )),
                    None => key = Some(chord),
                },
            }
        }
        commands.push(PluginCommand {
            plugin_id: plugin_id.to_owned(),
            id: cid,
            title,
            target,
            run,
            send,
            key,
            icon: c.get("icon").and_then(Value::as_str).map(str::to_owned),
            sidebar: c.get("sidebar") == Some(&Value::Bool(true)),
        });
    }
    Ok(commands)
}
