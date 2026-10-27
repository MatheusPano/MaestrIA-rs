use mx_core::json::Obj;
use serde_json::{Map, Value};


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickItem {
    pub value: String,
    pub label: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickRequest {
    pub title: String,
    pub placeholder: Option<String>,
    pub items: Vec<PickItem>,
}

fn opt(p: &Obj, key: &str) -> Option<String> {
    p.get(key).and_then(Value::as_str).map(str::to_owned)
}

impl PickRequest {
    pub fn parse(p: &Obj, plugin_name: &str) -> Self {
        let items = p
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
            .filter_map(|it| {
                let value = it.get("value")?.as_str()?.to_owned();
                Some(PickItem {
                    label: opt(it, "label").unwrap_or_else(|| value.clone()),
                    detail: opt(it, "detail"),
                    value,
                })
            })
            .collect();
        PickRequest {
            title: opt(p, "title").unwrap_or_else(|| plugin_name.to_owned()),
            placeholder: opt(p, "placeholder"),
            items,
        }
    }

    pub fn hint(&self) -> &str {
        self.placeholder.as_deref().unwrap_or("Filtrar")
    }

    pub fn filter(&self, query: &str) -> Vec<&PickItem> {
        let q = query.to_lowercase();
        let terms: Vec<&str> = q.split_whitespace().collect();
        self.items
            .iter()
            .filter(|it| {
                let hay = format!("{} {}", it.label, it.detail.as_deref().unwrap_or("")).to_lowercase();
                terms.iter().all(|t| hay.contains(t))
            })
            .collect()
    }
}

pub const PICK_EMPTY: &str = "Nada com esse nome";

pub fn cycle_highlight(current: usize, delta: i64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let n = len as i64;
    (((current as i64 + delta) % n + n) % n) as usize
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRequest {
    pub title: String,
    pub placeholder: Option<String>,
    pub value: Option<String>,
    pub prompt: Option<String>,
}

impl InputRequest {
    pub fn parse(p: &Obj, plugin_name: &str) -> Self {
        InputRequest {
            title: opt(p, "title").unwrap_or_else(|| plugin_name.to_owned()),
            placeholder: opt(p, "placeholder"),
            value: opt(p, "value"),
            prompt: opt(p, "prompt"),
        }
    }

    pub fn help(&self) -> &str {
        self.prompt.as_deref().unwrap_or("enter pra confirmar, esc pra cancelar")
    }

    pub fn answer(typed: &str) -> Option<String> {
        let t = typed.trim();
        (!t.is_empty()).then(|| t.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormAnswer {
    pub action: String,
    pub values: Vec<(String, String)>,
}

impl FormAnswer {
    pub fn to_json(&self) -> Value {
        let values: Map<String, Value> =
            self.values.iter().map(|(k, v)| (k.clone(), Value::from(v.clone()))).collect();
        let mut m = Map::new();
        m.insert("action".into(), Value::from(self.action.clone()));
        m.insert("values".into(), Value::Object(values));
        Value::Object(m)
    }
}
