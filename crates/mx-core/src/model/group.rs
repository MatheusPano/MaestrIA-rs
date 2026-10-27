use serde_json::{Value, json};

use crate::json::{Obj, get_str, is_true, str_of};
use crate::text::utf16_sum;

#[derive(Debug, Clone, PartialEq)]
pub struct PaneGroup {
    pub id: String,
    pub name: String,
    pub panes: Vec<Obj>,
    pub tree: Option<Value>,
}

impl PaneGroup {
    pub fn count(&self) -> usize {
        self.panes.len()
    }

    pub fn summary(&self) -> String {
        summarize(&self.panes)
    }

    pub fn where_(&self) -> String {
        let mut seen: Vec<&str> = Vec::new();
        for p in &self.panes {
            let root = if is_true(p, "loose") {
                "avulsos"
            } else {
                str_of(p, "folderRoot").unwrap_or("")
            };
            let Some(seg) = root.split('/').rev().find(|s| !s.is_empty()) else {
                continue;
            };
            if !seen.contains(&seg) {
                seen.push(seg);
            }
        }
        seen.join(" · ")
    }

    pub fn tint_index(&self, palette_len: usize) -> usize {
        tint_index_of(&self.id, palette_len)
    }

    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("id".into(), json!(self.id));
        m.insert("name".into(), json!(self.name));
        m.insert(
            "panes".into(),
            Value::Array(self.panes.iter().cloned().map(Value::Object).collect()),
        );
        if let Some(tree) = &self.tree
            && !tree.is_null()
        {
            m.insert("tree".into(), tree.clone());
        }
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Option<PaneGroup> {
        let m = v.as_object()?;
        let id = get_str(m, "id").typed()?.unwrap_or("").to_owned();
        let name = get_str(m, "name").typed()?.unwrap_or("").trim().to_owned();
        let panes: Vec<Obj> = match m.get("panes") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|p| p.as_object().cloned())
                .collect(),
            None | Some(Value::Null) => Vec::new(),
            Some(_) => return None,
        };
        if id.is_empty() || name.is_empty() || panes.is_empty() {
            return None;
        }
        let tree = m.get("tree").filter(|t| !t.is_null()).cloned();
        Some(PaneGroup {
            id,
            name,
            panes,
            tree,
        })
    }
}

pub fn tint_index_of(id: &str, palette_len: usize) -> usize {
    if palette_len == 0 {
        return 0;
    }
    (utf16_sum(id) % palette_len as u64) as usize
}

pub fn summarize(panes: &[Obj]) -> String {
    let count = |kind: &str| {
        panes
            .iter()
            .filter(|p| str_of(p, "kind") == Some(kind))
            .count()
    };
    let mut parts = Vec::new();
    for (kind, one, many) in [
        ("claude", "uma sessão", "sessões"),
        ("shell", "um terminal", "terminais"),
        ("reader", "um leitor", "leitores"),
    ] {
        match count(kind) {
            0 => {}
            1 => parts.push(one.to_owned()),
            n => parts.push(format!("{n} {many}")),
        }
    }
    match parts.len() {
        0 => String::new(),
        1 => parts.remove(0),
        n => format!("{} e {}", parts[..n - 1].join(", "), parts[n - 1]),
    }
}
