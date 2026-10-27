use mx_core::ports::TermKey;
use serde_json::Value;

use super::RpcError;

pub const MAX_KEYS: usize = 200;

pub fn parse_keys(raw: Option<&Value>) -> Option<Vec<TermKey>> {
    let Some(Value::Array(items)) = raw else { return None };
    if items.is_empty() || items.len() > MAX_KEYS {
        return None;
    }
    items
        .iter()
        .map(|item| match item {
            Value::String(name) => TermKey::by_name(name),
            Value::Object(m) => m.get("text").and_then(Value::as_str).map(|t| TermKey::Text(t.to_owned())),
            _ => None,
        })
        .collect()
}

pub fn bad_keys() -> RpcError {
    let names: Vec<&str> = TermKey::NAMED.iter().filter_map(TermKey::name).collect();
    RpcError::invalid(format!("`keys`: uma lista de {} ou {{ text }}", names.join(", ")))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn named_keys_and_text_objects_parse_in_order() {
        let keys = parse_keys(Some(&json!(["up", {"text": "oi"}, "enter"]))).unwrap();
        assert_eq!(keys, [TermKey::Up, TermKey::Text("oi".into()), TermKey::Enter]);
    }

    #[test]
    fn anything_else_makes_the_whole_list_invalid() {
        for bad in [json!([]), json!("up"), json!(["pgup"]), json!([{"text": 1}]), json!([1])] {
            assert_eq!(parse_keys(Some(&bad)), None, "{bad}");
        }
        assert_eq!(parse_keys(None), None);
        assert!(parse_keys(Some(&json!(vec!["tab"; 200]))).is_some());
        assert_eq!(parse_keys(Some(&json!(vec!["tab"; 201]))), None);
    }

    #[test]
    fn the_error_lists_every_key_name() {
        assert_eq!(
            bad_keys().message,
            "`keys`: uma lista de up, down, left, right, enter, escape, tab, space, backspace ou { text }"
        );
    }
}
