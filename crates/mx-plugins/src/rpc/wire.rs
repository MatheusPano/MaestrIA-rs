use serde_json::{Map, Value};

use super::RpcError;

fn envelope() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("jsonrpc".into(), Value::from("2.0"));
    m
}

pub fn request(id: i64, method: &str, params: Option<Value>) -> String {
    let mut m = envelope();
    m.insert("id".into(), Value::from(id));
    m.insert("method".into(), Value::from(method));
    if let Some(p) = params {
        m.insert("params".into(), p);
    }
    Value::Object(m).to_string()
}

pub fn notification(method: &str, params: Option<Value>) -> String {
    let mut m = envelope();
    m.insert("method".into(), Value::from(method));
    if let Some(p) = params {
        m.insert("params".into(), p);
    }
    Value::Object(m).to_string()
}

pub fn response(id: Value, outcome: Result<Value, RpcError>) -> String {
    let mut m = envelope();
    m.insert("id".into(), id);
    match outcome {
        Ok(result) => {
            m.insert("result".into(), result);
        }
        Err(e) => {
            let mut err = Map::new();
            err.insert("code".into(), Value::from(e.code));
            err.insert("message".into(), Value::from(e.message));
            m.insert("error".into(), Value::Object(err));
        }
    }
    Value::Object(m).to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Notification { method: String, params: Map<String, Value> },
    Request { id: Value, method: String, params: Map<String, Value> },
    Response { id: i64, outcome: Result<Value, RpcError> },
    Ignored,
    Garbage,
}

pub fn parse(line: &str) -> Incoming {
    if line.trim().is_empty() {
        return Incoming::Ignored;
    }
    let Ok(Value::Object(msg)) = serde_json::from_str::<Value>(line) else {
        return Incoming::Garbage;
    };
    let params = msg.get("params").and_then(Value::as_object).cloned().unwrap_or_default();
    let id = msg.get("id").filter(|v| !v.is_null()).cloned();
    if let Some(Value::String(method)) = msg.get("method") {
        let method = method.clone();
        return match id {
            None => Incoming::Notification { method, params },
            Some(id) => Incoming::Request { id, method, params },
        };
    }
    let Some(id) = id.as_ref().and_then(|v| v.as_i64().filter(|_| !v.is_f64())) else {
        return Incoming::Ignored;
    };
    let outcome = match msg.get("error") {
        Some(Value::Object(err)) => Err(RpcError::new(
            err.get("code").and_then(Value::as_f64).map_or(RpcError::INTERNAL, |c| c.trunc() as i64),
            err.get("message").and_then(Value::as_str).unwrap_or("erro sem mensagem"),
        )),
        _ => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
    };
    Incoming::Response { id, outcome }
}
