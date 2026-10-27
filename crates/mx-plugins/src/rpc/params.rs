use mx_core::json::Obj;
use serde_json::Value;

use super::RpcError;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RfwUpdate {
    pub library: Option<String>,
    pub root: Option<String>,
    pub data: Option<Obj>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BadgeUpdate {
    Unchanged,
    Clear,
    Set(String),
}

fn present(p: &Obj, key: &str) -> Option<Value> {
    p.get(key).filter(|v| !v.is_null()).cloned()
}

pub fn parse_rfw(p: &Obj) -> Result<Option<RfwUpdate>, RpcError> {
    let rfw = present(p, "rfw");
    let data = present(p, "data");
    if rfw.is_none() && data.is_none() {
        return Ok(None);
    }
    let rfw = match rfw {
        None => None,
        Some(Value::Object(m)) => Some(m),
        Some(_) => return Err(RpcError::invalid("\"rfw\" é { library, root? }")),
    };
    let data = match data {
        None => None,
        Some(Value::Object(m)) => Some(m),
        Some(_) => return Err(RpcError::invalid("\"data\" é um objeto")),
    };
    let text = |key: &str| -> Result<Option<String>, RpcError> {
        match rfw.as_ref().and_then(|r| r.get(key)) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(other) => Err(RpcError::internal(format!(
                "type '{}' is not a subtype of type 'String?' in type cast",
                dart_type(other)
            ))),
        }
    };
    Ok(Some(RfwUpdate { library: text("library")?, root: text("root")?, data }))
}

fn dart_type(v: &Value) -> &'static str {
    match v {
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "double",
        Value::Number(_) => "int",
        Value::Array(_) => "List<dynamic>",
        Value::Object(_) => "_Map<String, dynamic>",
        _ => "Null",
    }
}

pub(crate) fn blocks_from(raw: Option<&Value>) -> Vec<Obj> {
    match raw {
        Some(Value::Array(items)) => items.iter().filter_map(Value::as_object).cloned().collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn blocks_if_present(p: &Obj) -> Option<Vec<Obj>> {
    p.contains_key("blocks").then(|| blocks_from(p.get("blocks")))
}

pub(crate) fn text(p: &Obj, key: &str) -> Result<String, RpcError> {
    match p.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(RpcError::required(key)),
    }
}

pub(crate) fn opt_text(p: &Obj, key: &str) -> Option<String> {
    p.get(key).and_then(Value::as_str).map(str::to_owned)
}

pub(crate) fn opt_num(p: &Obj, key: &str) -> Option<f64> {
    p.get(key).and_then(Value::as_f64)
}

pub(crate) fn is_true(p: &Obj, key: &str) -> bool {
    p.get(key) == Some(&Value::Bool(true))
}

pub(crate) fn badge(p: &Obj) -> BadgeUpdate {
    match p.get("badge") {
        None => BadgeUpdate::Unchanged,
        Some(Value::Null) => BadgeUpdate::Clear,
        Some(Value::String(s)) if s.is_empty() => BadgeUpdate::Clear,
        Some(Value::String(s)) => BadgeUpdate::Set(s.clone()),
        Some(other) => BadgeUpdate::Set(dart_string(other)),
    }
}

pub(crate) fn dart_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.is_finite() => format!("{f:.1}"),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}
