use serde_json::{Value, json};

use crate::json::f64_value;
use crate::layout::tree::{Axis, PaneNode, normalize};

pub fn to_json(node: Option<&PaneNode>, index_of: &dyn Fn(&str) -> i64) -> Option<Value> {
    match node? {
        PaneNode::Leaf(id) => {
            let i = index_of(id);
            (i >= 0).then(|| json!(i))
        }
        PaneNode::Split(s) => {
            let mut children = Vec::new();
            let mut weights = Vec::new();
            for (child, w) in s.children.iter().zip(&s.weights) {
                if let Some(j) = to_json(Some(child), index_of) {
                    children.push(j);
                    weights.push(*w);
                }
            }
            match children.len() {
                0 => None,
                1 => children.pop(),
                _ => {
                    normalize(&mut weights);
                    let weights: Vec<Value> = weights.into_iter().map(f64_value).collect();
                    let mut m = serde_json::Map::new();
                    m.insert("axis".into(), json!(s.axis.name()));
                    m.insert("weights".into(), Value::Array(weights));
                    m.insert("children".into(), Value::Array(children));
                    Some(Value::Object(m))
                }
            }
        }
    }
}

pub fn from_json(json: &Value, tab_id_at: &dyn Fn(i64) -> Option<String>) -> Option<PaneNode> {
    if let Some(i) = json.as_i64().filter(|_| json.is_i64() || json.is_u64()) {
        return tab_id_at(i).map(PaneNode::Leaf);
    }
    let m = json.as_object()?;
    let raw = m
        .get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let saved = m
        .get("weights")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut children = Vec::new();
    let mut weights = Vec::new();
    for (i, child) in raw.iter().enumerate() {
        let Some(node) = from_json(child, tab_id_at) else {
            continue;
        };
        children.push(node);
        weights.push(saved.get(i).and_then(Value::as_f64).unwrap_or(1.0));
    }
    match children.len() {
        0 => None,
        1 => children.pop(),
        _ => {
            normalize(&mut weights);
            let axis = if m.get("axis").and_then(Value::as_str) == Some("column") {
                Axis::Column
            } else {
                Axis::Row
            };
            Some(PaneNode::split(axis, children, weights))
        }
    }
}

pub fn legacy_two_slot(left: Option<String>, right: Option<String>) -> Option<PaneNode> {
    let left = left?;
    Some(match right {
        Some(right) => PaneNode::split(
            Axis::Row,
            vec![PaneNode::Leaf(left), PaneNode::Leaf(right)],
            vec![0.5, 0.5],
        ),
        None => PaneNode::Leaf(left),
    })
}
