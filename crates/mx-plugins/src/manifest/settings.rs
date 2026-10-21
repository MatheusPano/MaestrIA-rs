use mx_core::json::Obj;
use serde_json::Value;

use super::{ManifestError, show};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingType {
    String,
    Number,
    Boolean,
    Color,
    Select,
}

impl SettingType {
    pub const ALL: [SettingType; 5] =
        [Self::String, Self::Number, Self::Boolean, Self::Color, Self::Select];

    pub fn name(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Color => "color",
            Self::Select => "select",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginSetting {
    pub id: String,
    pub title: String,
    pub kind: SettingType,
    pub description: String,
    pub default: Value,
    pub options: Vec<SettingOption>,
    pub group: Option<String>,
}

impl PluginSetting {
    pub fn parse(j: &Obj) -> Result<PluginSetting, ManifestError> {
        let id = match j.get("id") {
            Some(Value::String(s)) if !s.is_empty() => s.clone(),
            _ => return Err(ManifestError::new("cada configuração precisa de \"id\"")),
        };
        let raw_type = j.get("type").filter(|v| !v.is_null());
        let kind = match raw_type {
            None => Some(SettingType::String),
            Some(Value::String(t)) => SettingType::ALL.into_iter().find(|k| k.name() == t),
            Some(_) => None,
        };
        let Some(kind) = kind else {
            let names: Vec<&str> = SettingType::ALL.iter().map(|t| t.name()).collect();
            return Err(ManifestError::new(format!(
                "a configuração \"{id}\" tem \"type\": \"{}\" — vale {}",
                show(raw_type.unwrap_or(&Value::Null)),
                names.join(", ")
            )));
        };
        let mut options = Vec::new();
        if let Some(Value::Array(items)) = j.get("options") {
            for o in items {
                let option = match o {
                    Value::String(v) => SettingOption { value: v.clone(), label: v.clone() },
                    Value::Object(m) => match m.get("value") {
                        Some(Value::String(v)) => SettingOption {
                            value: v.clone(),
                            label: m.get("label").and_then(Value::as_str).unwrap_or(v).to_owned(),
                        },
                        _ => return Err(bad_option(&id)),
                    },
                    _ => return Err(bad_option(&id)),
                };
                options.push(option);
            }
        }
        if kind == SettingType::Select && options.is_empty() {
            return Err(ManifestError::new(format!(
                "a configuração \"{id}\" é select e não tem \"options\""
            )));
        }
        Ok(PluginSetting {
            title: j.get("title").and_then(Value::as_str).unwrap_or(&id).to_owned(),
            description: j.get("description").and_then(Value::as_str).unwrap_or("").to_owned(),
            default: j.get("default").cloned().unwrap_or(Value::Null),
            group: j.get("group").and_then(Value::as_str).map(str::to_owned),
            id,
            kind,
            options,
        })
    }

    pub fn fits(&self, v: &Value) -> bool {
        match self.kind {
            SettingType::String | SettingType::Color => v.is_string(),
            SettingType::Number => v.is_number(),
            SettingType::Boolean => v.is_boolean(),
            SettingType::Select => v.as_str().is_some_and(|s| self.options.iter().any(|o| o.value == s)),
        }
    }

    pub fn resolve(&self, saved: Option<&Value>) -> Value {
        if let Some(v) = saved.filter(|v| self.fits(v)) {
            return v.clone();
        }
        if self.fits(&self.default) {
            return self.default.clone();
        }
        match self.kind {
            SettingType::Boolean => Value::Bool(false),
            SettingType::Number => Value::from(0),
            SettingType::Select => Value::String(self.options[0].value.clone()),
            _ => Value::String(String::new()),
        }
    }
}

fn bad_option(id: &str) -> ManifestError {
    ManifestError::new(format!("a opção de \"{id}\" é um texto ou {{\"value\", \"label\"}}"))
}
