use serde_json::{Map, Value};

use super::Plugins;
use crate::catalog::PluginOrigin;

impl Plugins {
    pub fn load(&self, json: &Value) {
        let mut reg = self.reg();
        reg.disabled.clear();
        reg.settings.clear();
        reg.origins.clear();
        reg.manual_update.clear();
        reg.auto_update_off = json.get("autoUpdate") == Some(&Value::Bool(false));
        if let Some(Value::Array(ids)) = json.get("disabled") {
            reg.disabled.extend(ids.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        if let Some(Value::Array(ids)) = json.get("manualUpdate") {
            reg.manual_update.extend(ids.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        if let Some(Value::Object(all)) = json.get("origins") {
            for (id, o) in all {
                if let Some(o) = PluginOrigin::from_json(o) {
                    reg.origins.insert(id.clone(), o);
                }
            }
        }
        if let Some(Value::Object(all)) = json.get("settings") {
            for (id, values) in all {
                if values.is_object() {
                    reg.settings.insert(id.clone(), values.clone());
                }
            }
        }
    }

    pub fn to_json(&self) -> Value {
        let reg = self.reg();
        let mut out = Map::new();
        if !reg.disabled.is_empty() {
            out.insert("disabled".into(), reg.disabled.iter().cloned().map(Value::from).collect());
        }
        if reg.auto_update_off {
            out.insert("autoUpdate".into(), Value::Bool(false));
        }
        if !reg.manual_update.is_empty() {
            out.insert("manualUpdate".into(), reg.manual_update.iter().cloned().map(Value::from).collect());
        }
        if !reg.origins.is_empty() {
            let origins: Map<String, Value> = reg.origins.iter().map(|(id, o)| (id.clone(), o.to_json())).collect();
            out.insert("origins".into(), Value::Object(origins));
        }
        let settings: Map<String, Value> = reg
            .settings
            .iter()
            .filter(|(_, m)| m.as_object().is_some_and(|m| !m.is_empty()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !settings.is_empty() {
            out.insert("settings".into(), Value::Object(settings));
        }
        Value::Object(out)
    }

    pub fn settings_of(&self, id: &str) -> Map<String, Value> {
        let reg = self.reg();
        let empty = Map::new();
        let saved = reg.settings.get(id).and_then(Value::as_object).unwrap_or(&empty);
        let Some(m) = reg.entry(id).and_then(|e| e.manifest.clone()) else { return Map::new() };
        m.settings.iter().map(|s| (s.id.clone(), s.resolve(saved.get(&s.id)))).collect()
    }

    pub fn set_setting(&self, id: &str, key: &str, value: Option<Value>) {
        let conn = {
            let mut reg = self.reg();
            let slot = reg.settings.entry(id.to_owned()).or_insert_with(|| Value::Object(Map::new()));
            if !slot.is_object() {
                *slot = Value::Object(Map::new());
            }
            let Some(saved) = slot.as_object_mut() else { return };
            match value {
                Some(v) if !v.is_null() => {
                    saved.insert(key.to_owned(), v);
                }
                _ => {
                    saved.shift_remove(key);
                }
            }
            reg.entry(id).and_then(|e| e.live_conn()).filter(|c| c.ready())
        };
        if let Some(conn) = conn {
            let settings = Value::Object(self.settings_of(id));
            let mut params = Map::new();
            params.insert("settings".into(), settings);
            conn.notify("settings.changed", Some(Value::Object(params)));
        }
        self.changed();
    }
}

impl Plugins {
    pub fn auto_update(&self) -> bool {
        !self.reg().auto_update_off
    }

    pub fn set_auto_update(&self, on: bool) {
        self.reg().auto_update_off = !on;
    }

    pub fn manual_update(&self) -> std::collections::BTreeSet<String> {
        self.reg().manual_update.clone()
    }

    pub fn set_manual_update(&self, id: &str, manual: bool) {
        let mut reg = self.reg();
        if manual {
            reg.manual_update.insert(id.to_owned());
        } else {
            reg.manual_update.remove(id);
        }
    }

    pub fn origin_of(&self, id: &str) -> Option<PluginOrigin> {
        self.reg().origins.get(id).cloned()
    }
}
