use std::collections::BTreeMap;

use mx_core::json::Obj;
use serde_json::Value;

use super::console::PluginConsole;
use crate::rpc::RfwUpdate;

pub const DEFAULT_RFW_ROOT: &str = "root";
pub const EMPTY_VIEW: &str = "Nada pra mostrar ainda";
pub const MISSING_PLUGIN: &str = "O plugin não está mais instalado";

#[derive(Debug, Clone, PartialEq)]
pub struct PluginView {
    pub plugin_id: String,
    pub plugin_name: String,
    pub id: String,
    pub title: String,
    pub rfw_library: Option<String>,
    pub rfw_root: String,
    pub rfw_data: Obj,
    pub rfw_revision: u64,
    pub blocks: Vec<Obj>,
    pub revision: u64,
    pub consoles: BTreeMap<String, PluginConsole>,
    pub modal_from: Option<String>,
    pub modal_width: Option<f64>,
    pub modal_height: Option<f64>,
}

pub(crate) fn children(b: &Obj) -> Vec<&Obj> {
    match b.get("children") {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_object).collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn is_container(b: &Obj) -> bool {
    matches!(b.get("type").and_then(Value::as_str), Some("row" | "card" | "columns"))
}

impl PluginView {
    pub fn new(plugin_id: &str, plugin_name: &str, id: &str, title: &str, blocks: Vec<Obj>) -> Self {
        let mut view = PluginView {
            plugin_id: plugin_id.to_owned(),
            plugin_name: plugin_name.to_owned(),
            id: id.to_owned(),
            title: title.to_owned(),
            rfw_library: None,
            rfw_root: DEFAULT_RFW_ROOT.to_owned(),
            rfw_data: Obj::new(),
            rfw_revision: 0,
            blocks,
            revision: 0,
            consoles: BTreeMap::new(),
            modal_from: None,
            modal_width: None,
            modal_height: None,
        };
        view.adopt_consoles();
        view
    }

    pub fn subtitle(&self) -> String {
        format!("Plugin · {}", self.plugin_name)
    }

    pub fn uses_rfw(&self) -> bool {
        self.rfw_library.is_some()
    }

    pub fn set_rfw(&mut self, update: &RfwUpdate) {
        if let Some(lib) = &update.library {
            self.rfw_library = Some(lib.clone());
        }
        if let Some(root) = &update.root {
            self.rfw_root = root.clone();
        }
        if let Some(data) = &update.data {
            for (k, v) in data {
                self.rfw_data.insert(k.clone(), v.clone());
            }
            self.rfw_revision += 1;
        }
    }

    pub fn set_blocks(&mut self, blocks: Vec<Obj>) {
        self.blocks = blocks;
        self.revision += 1;
        self.adopt_consoles();
    }

    pub fn apply_open(&mut self, title: &str, blocks: Vec<Obj>, rfw: Option<&RfwUpdate>) {
        self.title = title.to_owned();
        self.set_blocks(blocks);
        if let Some(r) = rfw {
            self.set_rfw(r);
        }
    }

    pub fn apply_update(&mut self, title: Option<&str>, blocks: Option<Vec<Obj>>, rfw: Option<&RfwUpdate>) {
        if let Some(t) = title {
            self.title = t.to_owned();
        }
        if let Some(r) = rfw {
            self.set_rfw(r);
        }
        if let Some(b) = blocks {
            self.rfw_library = None;
            self.set_blocks(b);
        }
    }

    pub fn set_modal(&mut self, from: Option<String>, width: Option<f64>, height: Option<f64>) {
        self.modal_from = from;
        self.modal_width = width;
        self.modal_height = height;
    }

    pub fn switches_to(&self, blocks: bool, rfw: Option<&RfwUpdate>) -> bool {
        blocks || (rfw.is_some_and(|r| r.library.is_some()) && self.rfw_library.is_none())
    }

    pub fn console(&mut self, id: &str) -> &mut PluginConsole {
        self.consoles.entry(id.to_owned()).or_default()
    }

    fn adopt_consoles(&mut self) {
        let mut found: Vec<(String, Option<usize>, Option<Value>)> = Vec::new();
        fn visit(blocks: &[&Obj], found: &mut Vec<(String, Option<usize>, Option<Value>)>) {
            for b in blocks {
                if is_container(b) {
                    visit(&children(b), found);
                }
                if b.get("type").and_then(Value::as_str) != Some("console") {
                    continue;
                }
                let Some(id) = b.get("id").and_then(Value::as_str) else { continue };
                let max = b.get("max").and_then(Value::as_f64).map(|n| n.trunc().max(0.0) as usize);
                let lines = b.contains_key("lines").then(|| b.get("lines").cloned().unwrap_or(Value::Null));
                found.push((id.to_owned(), max, lines));
            }
        }
        let top: Vec<&Obj> = self.blocks.iter().collect();
        visit(&top, &mut found);
        for (id, max, lines) in &found {
            let c = self.console(id);
            if let Some(m) = max {
                c.max = *m;
            }
            if let Some(l) = lines {
                c.replace(l);
            }
        }
        self.consoles.retain(|k, _| found.iter().any(|(id, _, _)| id == k));
    }

    pub fn terminal_tabs(&self) -> Vec<String> {
        fn visit(blocks: &[&Obj], out: &mut Vec<String>) {
            for b in blocks {
                if is_container(b) {
                    visit(&children(b), out);
                }
                if b.get("type").and_then(Value::as_str) == Some("terminal")
                    && let Some(t) = b.get("tabId").and_then(Value::as_str)
                {
                    out.push(t.to_owned());
                }
            }
        }
        let mut out = Vec::new();
        visit(&self.blocks.iter().collect::<Vec<_>>(), &mut out);
        out
    }

    pub fn expanding_index(&self) -> Option<usize> {
        self.blocks.iter().position(|b| {
            matches!(b.get("type").and_then(Value::as_str), Some("console" | "terminal"))
                && b.get("expand") == Some(&Value::Bool(true))
        })
    }
}
