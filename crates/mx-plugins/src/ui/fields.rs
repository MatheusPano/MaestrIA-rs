use std::collections::{BTreeMap, BTreeSet};

use mx_core::json::Obj;
use serde_json::{Map, Value};

use super::view::{children, is_container};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FieldState {
    texts: BTreeMap<String, String>,
    picks: BTreeMap<String, Value>,
    sent: BTreeMap<String, Value>,
    order: Vec<String>,
}

impl FieldState {
    pub fn sync(&mut self, blocks: &[Obj]) {
        let mut seen = BTreeSet::new();
        let top: Vec<&Obj> = blocks.iter().collect();
        self.visit(&top, &mut seen);
        self.texts.retain(|k, _| seen.contains(k));
        self.picks.retain(|k, _| seen.contains(k));
        self.sent.retain(|k, _| seen.contains(k));
        self.order.retain(|k| seen.contains(k));
    }

    fn remember(&mut self, id: &str) {
        if !self.order.iter().any(|o| o == id) {
            self.order.push(id.to_owned());
        }
    }

    fn visit(&mut self, blocks: &[&Obj], seen: &mut BTreeSet<String>) {
        for b in blocks {
            let id = b.get("id").and_then(Value::as_str);
            match (b.get("type").and_then(Value::as_str), id) {
                (Some("input"), Some(id)) => {
                    seen.insert(id.to_owned());
                    self.remember(id);
                    let value = b.get("value").and_then(Value::as_str).unwrap_or("").to_owned();
                    let sent = Value::String(value.clone());
                    let changed = self.sent.get(id).is_some_and(|s| *s != sent);
                    if !self.texts.contains_key(id) || changed {
                        self.texts.insert(id.to_owned(), value);
                    }
                    self.sent.insert(id.to_owned(), sent);
                }
                (Some("checkbox" | "select" | "calendar"), Some(id)) => {
                    seen.insert(id.to_owned());
                    self.remember(id);
                    let value = b.get("value").cloned().unwrap_or(Value::Null);
                    let changed = self.sent.get(id) != Some(&value);
                    if !self.picks.contains_key(id) || changed {
                        self.picks.insert(id.to_owned(), value.clone());
                    }
                    self.sent.insert(id.to_owned(), value);
                }
                _ if is_container(b) => self.visit(&children(b), seen),
                _ => {}
            }
        }
    }

    pub fn text(&self, id: &str) -> Option<&str> {
        self.texts.get(id).map(String::as_str)
    }

    pub fn pick(&self, id: &str) -> Option<&Value> {
        self.picks.get(id)
    }

    pub fn set_text(&mut self, id: &str, text: &str) {
        if let Some(t) = self.texts.get_mut(id) {
            *t = text.to_owned();
        }
    }

    pub fn set_pick(&mut self, id: &str, value: Value) {
        if self.picks.contains_key(id) {
            self.picks.insert(id.to_owned(), value);
        }
    }

    pub fn values(&self) -> Map<String, Value> {
        let mut out = Map::new();
        for id in &self.order {
            if let Some(t) = self.texts.get(id) {
                out.insert(id.clone(), Value::String(t.clone()));
            }
        }
        for id in &self.order {
            if let Some(v) = self.picks.get(id) {
                out.insert(id.clone(), v.clone());
            }
        }
        out
    }
}
