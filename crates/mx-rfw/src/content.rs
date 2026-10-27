use crate::ast::Part;
use crate::value::{MISSING, Map, Value};

pub const THEME_KEY: &str = "theme";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DynamicContent {
    root: Map,
}

impl DynamicContent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_json(json: &serde_json::Value) -> Self {
        let mut content = Self::new();
        if let Some(obj) = json.as_object() {
            for (k, v) in obj {
                content.update(k, Value::from_json(v));
            }
        }
        content
    }

    pub fn root(&self) -> &Map {
        &self.root
    }

    pub fn update(&mut self, key: &str, value: Value) {
        if value.is_missing() {
            self.root.shift_remove(key);
        } else {
            self.root.insert(key.to_owned(), value);
        }
    }

    pub fn update_all(&mut self, map: &Map) {
        for (k, v) in map {
            let v = if matches!(v, Value::Null) { Value::Missing } else { v.clone() };
            self.update(k, v);
        }
    }

    pub fn merge_plugin_data(&mut self, data: &serde_json::Map<String, serde_json::Value>) {
        for (k, v) in data {
            if k != THEME_KEY {
                self.update(k, Value::from_json(v));
            }
        }
    }

    pub fn set_theme(&mut self, theme: Value) {
        self.update(THEME_KEY, theme);
    }

    pub fn lookup(&self, parts: &[Part]) -> &Value {
        let Some((Part::Key(first), rest)) = parts.split_first() else { return &MISSING };
        let mut current = self.root.get(first).unwrap_or(&MISSING);
        for part in rest {
            current = match (current, part) {
                (Value::Map(m), Part::Key(k)) => m.get(k).unwrap_or(&MISSING),
                (Value::List(l), Part::Index(i)) => {
                    usize::try_from(*i).ok().and_then(|i| l.get(i)).unwrap_or(&MISSING)
                }
                _ => &MISSING,
            };
        }
        current
    }
}
