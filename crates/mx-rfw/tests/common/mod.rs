#![allow(dead_code)]

use std::path::PathBuf;

use mx_rfw::{DynamicContent, Map, Node, Value, Widget};

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name)).unwrap()
}

pub fn theme() -> Value {
    let mut m = Map::new();
    m.insert("dark".into(), Value::Bool(true));
    let colors = [
        ("canvas", 0xFF10_1010u32), ("bg", 0xFF15_1515), ("sidebar", 0xFF1A_1A1A), ("hover", 0xFF22_2222),
        ("active", 0xFF2A_2A2A), ("border", 0xFF33_3333), ("fg", 0xFFEE_EEEE), ("dim", 0xFFAA_AAAA),
        ("faint", 0xFF77_7777), ("accent", 0xFFF2_762E), ("onAccent", 0xFFFF_FFFF), ("green", 0xFF00_FF00),
        ("yellow", 0xFFFF_FF00), ("red", 0xFFFF_0000), ("purple", 0xFF80_00FF), ("blue", 0xFF00_00FF),
        ("cyan", 0xFF00_FFFF), ("magenta", 0xFFFF_00FF),
    ];
    for (k, v) in colors {
        m.insert(k.into(), Value::Int(i64::from(v)));
    }
    m.insert("mono".into(), Value::from("JetBrains Mono"));
    Value::Map(m)
}

pub fn content(json: &str) -> DynamicContent {
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    let mut c = DynamicContent::new();
    c.merge_plugin_data(parsed.as_object().unwrap());
    c.set_theme(theme());
    c
}

pub fn nodes(root: &Widget) -> Vec<&Node> {
    let mut out = Vec::new();
    root.walk(&mut |w| {
        if let Widget::Node(n) = w {
            out.push(n);
        }
    });
    out
}

pub fn texts(root: &Widget) -> Vec<String> {
    nodes(root)
        .into_iter()
        .filter(|n| n.name == "Text")
        .map(|n| mx_rfw::widgets::core_layout::Text::from_node(n).text)
        .collect()
}

pub fn problems(root: &Widget) -> Vec<String> {
    let mut out = Vec::new();
    root.walk(&mut |w| match w {
        Widget::Error(e) => out.push(e.clone()),
        Widget::Node(n) => {
            let required = ["Expanded", "Flexible", "FractionallySizedBox", "Positioned", "Tooltip"];
            if required.contains(&n.name.as_str())
                && let Err(e) = n.child("child")
            {
                out.push(e);
            }
            if ["Column", "Row", "Stack", "Wrap"].contains(&n.name.as_str()) {
                out.extend(n.child_list("children").into_iter().filter_map(Result::err));
            }
            if ["ListView", "GridView"].contains(&n.name.as_str()) {
                out.extend(n.indexed_children("children").into_iter().filter_map(Result::err));
            }
        }
    });
    out
}

pub fn find(root: &Widget, pred: impl Fn(&Node) -> bool) -> Vec<&Node> {
    nodes(root).into_iter().filter(|n| pred(n)).collect()
}
