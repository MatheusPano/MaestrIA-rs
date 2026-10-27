mod fetch;
mod updates;

#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::collections::BTreeSet;

use mx_core::version::compare_versions;
use serde_json::{Map, Value};

use crate::{MAESTRIA_COMPAT, PLUGIN_API};

pub use fetch::{git_head, sha256_file};
pub use updates::{EVERY, FIRST_DELAY, PluginUpdates, fetch_icon};

pub const OFFICIAL_CATALOG: &str = "https://github.com/MatheusPano/maestria-plugins/releases/download/catalog/catalog.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PluginKind {
    Theme,
    Feature,
}

impl PluginKind {
    pub const ALL: [PluginKind; 2] = [PluginKind::Theme, PluginKind::Feature];

    pub fn by_id(id: &str) -> Option<PluginKind> {
        PluginKind::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn id(self) -> &'static str {
        match self {
            PluginKind::Theme => "theme",
            PluginKind::Feature => "feature",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PluginKind::Theme => "Tema",
            PluginKind::Feature => "Recurso",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PluginOrigin {
    Catalog(String),
    Git { url: String, git_ref: Option<String> },
}

impl PluginOrigin {
    pub fn catalog(&self) -> Option<&str> {
        match self {
            PluginOrigin::Catalog(url) => Some(url),
            PluginOrigin::Git { .. } => None,
        }
    }

    pub fn git(&self) -> Option<(&str, Option<&str>)> {
        match self {
            PluginOrigin::Git { url, git_ref } => Some((url, git_ref.as_deref())),
            PluginOrigin::Catalog(_) => None,
        }
    }

    pub fn from_json(v: &Value) -> Option<PluginOrigin> {
        let m = v.as_object()?;
        if let Some(url) = m.get("catalog").and_then(Value::as_str) {
            return Some(PluginOrigin::Catalog(url.to_owned()));
        }
        let url = m.get("git").and_then(Value::as_str)?;
        Some(PluginOrigin::Git { url: url.to_owned(), git_ref: m.get("ref").and_then(Value::as_str).map(str::to_owned) })
    }

    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        match self {
            PluginOrigin::Catalog(url) => {
                m.insert("catalog".into(), url.clone().into());
            }
            PluginOrigin::Git { url, git_ref } => {
                m.insert("git".into(), url.clone().into());
                if let Some(r) = git_ref {
                    m.insert("ref".into(), r.clone().into());
                }
            }
        }
        Value::Object(m)
    }

    pub fn describe(&self) -> String {
        match self {
            PluginOrigin::Catalog(url) if url == OFFICIAL_CATALOG => "Vem do catálogo oficial".into(),
            PluginOrigin::Catalog(url) => format!("Vem do catálogo {url}"),
            PluginOrigin::Git { url, git_ref: None } => format!("Vem de {url}"),
            PluginOrigin::Git { url, git_ref: Some(r) } => format!("Vem de {url} ({r})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub api: i64,
    pub min_maestria: Option<String>,
    pub permissions: Vec<String>,
    pub kinds: BTreeSet<PluginKind>,
    pub icon: Option<String>,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub git: Option<String>,
    pub git_ref: Option<String>,
}

fn text(m: &Map<String, Value>, key: &str) -> Option<String> {
    m.get(key).and_then(Value::as_str).map(str::to_owned)
}

impl CatalogEntry {
    pub fn from_json(v: &Value) -> Option<CatalogEntry> {
        let m = v.as_object()?;
        let (id, version) = (text(m, "id")?, text(m, "version")?);
        let (url, sha, git) = (text(m, "url"), text(m, "sha256"), text(m, "git"));
        if !(url.is_some() && sha.is_some()) && git.is_none() {
            return None;
        }
        let (api, min_maestria) = match m.get("maestria") {
            Some(Value::Number(n)) => (n.as_f64().map_or(PLUGIN_API, |f| f.trunc() as i64), None),
            Some(Value::String(s)) => (PLUGIN_API, Some(s.clone())),
            _ => (PLUGIN_API, None),
        };
        let strings = |key: &str| -> Vec<String> {
            m.get(key).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
        };
        let zip = git.is_none();
        Some(CatalogEntry {
            name: text(m, "name").unwrap_or_else(|| id.clone()),
            description: text(m, "description").unwrap_or_default(),
            author: text(m, "author").unwrap_or_default(),
            api,
            min_maestria,
            permissions: strings("permissions"),
            kinds: strings("kinds").iter().filter_map(|k| PluginKind::by_id(k)).collect(),
            icon: text(m, "icon"),
            url: url.filter(|_| zip),
            sha256: sha.filter(|_| zip).map(|s| s.to_lowercase()),
            git,
            git_ref: text(m, "ref"),
            id,
            version,
        })
    }

    pub fn fits(&self) -> bool {
        self.api <= PLUGIN_API
            && self.min_maestria.as_deref().is_none_or(|min| compare_versions(min, MAESTRIA_COMPAT) != Ordering::Greater)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginCatalog {
    pub url: String,
    pub plugins: Vec<CatalogEntry>,
}

impl PluginCatalog {
    pub fn parse(url: &str, body: &str) -> Result<PluginCatalog, String> {
        let json: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        let Some(list) = json.as_object().and_then(|m| m.get("plugins")).and_then(Value::as_array) else {
            return Err("não é um catálogo".into());
        };
        Ok(PluginCatalog { url: url.to_owned(), plugins: list.iter().filter_map(CatalogEntry::from_json).collect() })
    }

    pub fn by_id(&self, id: &str) -> Option<&CatalogEntry> {
        self.plugins.iter().find(|e| e.id == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginUpdate {
    pub id: String,
    pub from: String,
    pub to: String,
    pub origin: PluginOrigin,
    pub entry: Option<CatalogEntry>,
    pub asks_more: bool,
}

impl PluginUpdate {
    pub fn newer(&self) -> bool {
        compare_versions(&self.to, &self.from) == Ordering::Greater
    }
}
