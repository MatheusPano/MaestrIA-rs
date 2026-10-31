use serde_json::{Map, Value, json};

use super::plugin_tools::{NoTools, PluginTools};
use super::sink::SuggestionSink;
use super::tools::{INSTRUCTIONS, dismiss, suggest, tools_json};
use super::{DISMISS_TOOL, PROTOCOL_VERSION, SERVER_NAME, SUGGEST_TOOL};

pub fn parse_error() -> Value {
    error(Value::Null, -32700, "parse error")
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

pub async fn handle_body(
    body: &[u8],
    tab_id: &str,
    sink: &dyn SuggestionSink,
    tools: &dyn PluginTools,
) -> Option<Value> {
    let Ok(text) = std::str::from_utf8(body) else {
        return Some(parse_error());
    };
    match serde_json::from_str::<Value>(text) {
        Ok(message) => handle_with(&message, tab_id, sink, tools).await,
        Err(_) => Some(parse_error()),
    }
}

pub async fn handle(body: &Value, tab_id: &str, sink: &dyn SuggestionSink) -> Option<Value> {
    handle_with(body, tab_id, sink, &NoTools).await
}

pub async fn handle_with(
    body: &Value,
    tab_id: &str,
    sink: &dyn SuggestionSink,
    tools: &dyn PluginTools,
) -> Option<Value> {
    if let Value::Array(batch) = body {
        let mut answers = Vec::new();
        for message in batch {
            if let Some(answer) = one(message, tab_id, sink, tools).await {
                answers.push(answer);
            }
        }
        return (!answers.is_empty()).then_some(Value::Array(answers));
    }
    one(body, tab_id, sink, tools).await
}

async fn one(
    message: &Value,
    tab_id: &str,
    sink: &dyn SuggestionSink,
    tools: &dyn PluginTools,
) -> Option<Value> {
    let Value::Object(message) = message else {
        return Some(error(Value::Null, -32600, "invalid request"));
    };
    let id = message.get("id").filter(|id| !id.is_null())?.clone();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Some(error(id, -32600, "invalid request"));
    };
    let empty = Map::new();
    let params = message
        .get("params")
        .and_then(Value::as_object)
        .unwrap_or(&empty);

    Some(match method {
        "initialize" => result(
            id,
            json!({
                "protocolVersion": params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(PROTOCOL_VERSION),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": SERVER_NAME, "version": "1.0.0"},
                "instructions": INSTRUCTIONS,
            }),
        ),
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({"tools": all_tools(tools)})),
        "tools/call" => call(id, params, tab_id, sink, tools).await,
        other => error(id, -32601, &format!("method not found: {other}")),
    })
}

fn all_tools(plugins: &dyn PluginTools) -> Value {
    let mut all = match tools_json() {
        Value::Array(list) => list,
        _ => Vec::new(),
    };
    all.extend(plugins.list());
    Value::Array(all)
}

async fn call(
    id: Value,
    params: &Map<String, Value>,
    tab_id: &str,
    sink: &dyn SuggestionSink,
    tools: &dyn PluginTools,
) -> Value {
    let empty = Map::new();
    let args = params
        .get("arguments")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    match params.get("name") {
        Some(Value::String(name)) if name == SUGGEST_TOOL => {
            result(id, suggest(args, tab_id, sink).await)
        }
        Some(Value::String(name)) if name == DISMISS_TOOL => {
            result(id, dismiss(args, tab_id, sink).await)
        }
        Some(Value::String(name)) => {
            let args = Value::Object(args.clone());
            match tools.call(tab_id.to_owned(), name.clone(), args).await {
                Some(answer) => result(id, answer),
                None => error(id, -32602, &format!("unknown tool: {name}")),
            }
        }
        name => {
            let shown = match name {
                None | Some(Value::Null) => "null".to_owned(),
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            };
            error(id, -32602, &format!("unknown tool: {shown}"))
        }
    }
}
