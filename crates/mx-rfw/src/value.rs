use std::fmt;

use indexmap::IndexMap;
use serde_json::Value as Json;

use crate::error::dart_double;
use crate::tree::{Event, Widget};

pub type Map = IndexMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    #[default]
    Missing,
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    String(String),
    List(Vec<Value>),
    Map(Map),
    Widget(Box<Widget>),
    Event(Box<Event>),
}

pub static MISSING: Value = Value::Missing;

impl Value {
    pub fn get(&self, key: &str) -> &Value {
        match self {
            Value::Map(m) => m.get(key).unwrap_or(&MISSING),
            _ => &MISSING,
        }
    }

    pub fn at(&self, index: usize) -> &Value {
        match self {
            Value::List(l) => l.get(index).unwrap_or(&MISSING),
            _ => &MISSING,
        }
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, Value::Missing)
    }

    pub fn as_int(&self) -> Option<i64> {
        if let Value::Int(v) = self { Some(*v) } else { None }
    }

    pub fn as_double(&self) -> Option<f64> {
        if let Value::Double(v) = self { Some(*v) } else { None }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Value::Bool(v) = self { Some(*v) } else { None }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Value::String(v) = self { Some(v) } else { None }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        if let Value::List(v) = self { Some(v) } else { None }
    }

    pub fn as_map(&self) -> Option<&Map> {
        if let Value::Map(v) = self { Some(v) } else { None }
    }

    pub fn as_widget(&self) -> Option<&Widget> {
        if let Value::Widget(v) = self { Some(v) } else { None }
    }

    pub fn as_event(&self) -> Option<&Event> {
        if let Value::Event(v) = self { Some(v) } else { None }
    }

    pub fn len(&self) -> usize {
        self.as_list().map_or(0, <[Value]>::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn from_json(json: &Json) -> Value {
        match json {
            Json::Null => Value::Null,
            Json::Bool(b) => Value::Bool(*b),
            Json::Number(n) => match n.as_i64() {
                Some(i) => Value::Int(i),
                None => Value::Double(n.as_f64().unwrap_or(f64::NAN)),
            },
            Json::String(s) => Value::String(s.clone()),
            Json::Array(a) => Value::List(a.iter().map(Value::from_json).collect()),
            Json::Object(o) => Value::Map(o.iter().map(|(k, v)| (k.clone(), Value::from_json(v))).collect()),
        }
    }

    pub fn to_json(&self) -> Option<Json> {
        Some(match self {
            Value::Missing | Value::Widget(_) | Value::Event(_) => return None,
            Value::Null => Json::Null,
            Value::Bool(b) => Json::Bool(*b),
            Value::Int(i) => Json::from(*i),
            Value::Double(d) => serde_json::Number::from_f64(*d).map_or(Json::Null, Json::Number),
            Value::String(s) => Json::String(s.clone()),
            Value::List(l) => Json::Array(l.iter().map(|v| v.to_json().unwrap_or(Json::Null)).collect()),
            Value::Map(m) => Json::Object(map_to_json(m)),
        })
    }

    pub(crate) fn matches_case(&self, key: &Value) -> bool {
        match (self, key) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Double(a), Value::Double(b)) => a == b,
            (Value::Int(i), Value::Double(d)) | (Value::Double(d), Value::Int(i)) => (*i as f64) == *d,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            _ => false,
        }
    }
}

pub fn map_to_json(map: &Map) -> serde_json::Map<String, Json> {
    map.iter().filter_map(|(k, v)| v.to_json().map(|j| (k.clone(), j))).collect()
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Missing => f.write_str("<missing>"),
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Double(d) => f.write_str(&dart_double(*d)),
            Value::String(s) => f.write_str(s),
            Value::List(l) => {
                f.write_str("[")?;
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{v}")?;
                }
                f.write_str("]")
            }
            Value::Map(m) => write_map(f, m),
            Value::Widget(w) => write!(f, "{w}"),
            Value::Event(e) => {
                write!(f, "event {} ", e.name)?;
                write_map(f, &e.args)
            }
        }
    }
}

fn write_map(f: &mut fmt::Formatter<'_>, m: &Map) -> fmt::Result {
    f.write_str("{")?;
    for (i, (k, v)) in m.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{k}: {v}")?;
    }
    f.write_str("}")
}

impl From<&Json> for Value {
    fn from(json: &Json) -> Self {
        Value::from_json(json)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Double(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
