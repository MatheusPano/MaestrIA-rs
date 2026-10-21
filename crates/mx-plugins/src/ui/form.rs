use mx_core::json::Obj;
use serde_json::Value;

use super::dialogs::FormAnswer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    pub id: String,
    pub select: bool,
    pub label: Option<String>,
    pub placeholder: Option<String>,
    pub value: String,
    pub half: bool,
    pub required: bool,
    pub options: Vec<(String, String)>,
    pub show_if: Option<(String, String)>,
}

impl FormField {
    pub fn visible_with(&self, values: &[(String, String)]) -> bool {
        match &self.show_if {
            None => true,
            Some((field, want)) => values.iter().any(|(k, v)| k == field && v == want),
        }
    }

    pub fn initial(&self) -> String {
        if !self.select || self.options.iter().any(|(v, _)| *v == self.value) {
            return self.value.clone();
        }
        self.options.first().map(|(v, _)| v.clone()).unwrap_or_default()
    }

    pub fn missing_text(&self) -> String {
        format!("falta {}", self.label.as_deref().unwrap_or("este campo"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormButton {
    pub action: String,
    pub label: String,
    pub style: String,
}

impl FormButton {
    pub fn danger(&self) -> bool {
        self.style == "danger"
    }

    pub fn primary(&self) -> bool {
        self.style == "primary"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginForm {
    pub title: String,
    pub fields: Vec<FormField>,
    pub buttons: Vec<FormButton>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormPress {
    Submit(FormAnswer),
    Missing { field: String, text: String },
}

fn opt(m: &Obj, key: &str) -> Option<String> {
    m.get(key).and_then(Value::as_str).map(str::to_owned)
}

impl PluginForm {
    pub const CANCEL: &'static str = "cancelar";

    pub fn parse(p: &Obj, fallback_title: &str) -> Result<PluginForm, String> {
        let mut fields: Vec<FormField> = Vec::new();
        for raw in p.get("fields").and_then(Value::as_array).into_iter().flatten() {
            let Value::Object(f) = raw else { return Err("cada campo é um objeto".into()) };
            let id = match f.get("id") {
                Some(Value::String(s)) if !s.is_empty() => s.clone(),
                _ => return Err("campo sem \"id\"".into()),
            };
            if fields.iter().any(|o| o.id == id) {
                return Err(format!("campo repetido: \"{id}\""));
            }
            let kind = match f.get("type") {
                None | Some(Value::Null) => "text".to_owned(),
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            };
            if kind != "text" && kind != "select" {
                return Err(format!("o campo \"{id}\" tem \"type\": \"{kind}\" — vale \"text\" ou \"select\""));
            }
            let options = f
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|o| match o {
                    Value::String(v) => Some((v.clone(), v.clone())),
                    Value::Object(m) => {
                        let v = m.get("value")?.as_str()?.to_owned();
                        Some((v.clone(), opt(m, "label").unwrap_or(v)))
                    }
                    _ => None,
                })
                .collect();
            let show_if = f.get("showIf").and_then(Value::as_object).and_then(|s| {
                Some((s.get("field")?.as_str()?.to_owned(), s.get("value")?.as_str()?.to_owned()))
            });
            let value = match f.get("value") {
                Some(Value::String(v)) => v.clone(),
                Some(n @ Value::Number(_)) => crate::rpc::dart_string(n),
                _ => String::new(),
            };
            fields.push(FormField {
                select: kind == "select",
                label: opt(f, "label"),
                placeholder: opt(f, "placeholder"),
                value,
                half: f.get("half") == Some(&Value::Bool(true)),
                required: f.get("required") == Some(&Value::Bool(true)),
                options,
                show_if,
                id,
            });
        }
        if fields.is_empty() {
            return Err("\"fields\" precisa de ao menos um campo".into());
        }
        let mut buttons: Vec<FormButton> = p
            .get("buttons")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
            .filter_map(|b| {
                let action = b.get("action")?.as_str().filter(|a| !a.is_empty())?.to_owned();
                Some(FormButton {
                    label: opt(b, "label").unwrap_or_else(|| action.clone()),
                    style: opt(b, "style").unwrap_or_default(),
                    action,
                })
            })
            .collect();
        if buttons.is_empty() {
            buttons.push(FormButton { action: "ok".into(), label: "ok".into(), style: "primary".into() });
        }
        Ok(PluginForm {
            title: opt(p, "title").unwrap_or_else(|| fallback_title.to_owned()),
            fields,
            buttons,
            error: opt(p, "error").map(|e| e.trim().to_owned()).filter(|e| !e.is_empty()),
        })
    }

    pub fn initial_values(&self) -> Vec<(String, String)> {
        self.fields.iter().map(|f| (f.id.clone(), f.initial())).collect()
    }

    pub fn rows<'a>(&'a self, values: &[(String, String)]) -> Vec<Vec<&'a FormField>> {
        let shown: Vec<&FormField> = self.fields.iter().filter(|f| f.visible_with(values)).collect();
        let mut rows = Vec::new();
        let mut i = 0;
        while i < shown.len() {
            let f = shown[i];
            if f.half && i + 1 < shown.len() && shown[i + 1].half {
                rows.push(vec![f, shown[i + 1]]);
                i += 2;
            } else {
                rows.push(vec![f]);
                i += 1;
            }
        }
        rows
    }

    pub fn press(&self, button: &FormButton, values: &[(String, String)]) -> FormPress {
        if !button.danger() {
            let value_of = |id: &str| values.iter().find(|(k, _)| k == id).map_or("", |(_, v)| v.as_str());
            if let Some(f) = self
                .fields
                .iter()
                .find(|f| f.required && f.visible_with(values) && value_of(&f.id).trim().is_empty())
            {
                return FormPress::Missing { field: f.id.clone(), text: f.missing_text() };
            }
        }
        FormPress::Submit(FormAnswer { action: button.action.clone(), values: values.to_vec() })
    }

    pub fn enter_button(&self) -> Option<&FormButton> {
        self.buttons.iter().find(|b| b.primary()).or_else(|| self.buttons.iter().rev().find(|b| !b.danger()))
    }
}
