use mx_core::json::Obj;
use serde_json::Value;

use super::blocks::s;

fn on(b: &Obj, key: &str) -> bool {
    b.get(key) == Some(&Value::Bool(true))
}

#[derive(Debug, Clone, PartialEq)]
pub enum MenuEntry {
    Divider,
    Item { action: String, label: String, icon: Option<String>, tone: Option<String>, disabled: bool },
    Submenu { label: String, icon: Option<String>, children: Vec<(String, String, Option<String>)> },
}

pub fn menu_of(raw: &[&Obj]) -> Vec<MenuEntry> {
    raw.iter()
        .filter_map(|m| {
            if m.get("type").and_then(Value::as_str) == Some("divider") {
                return Some(MenuEntry::Divider);
            }
            if let Some(Value::Array(kids)) = m.get("children") {
                let children = kids
                    .iter()
                    .filter_map(Value::as_object)
                    .filter_map(|c| {
                        let action = s(c, "action")?;
                        Some((action.clone(), s(c, "label").unwrap_or(action), s(c, "tone")))
                    })
                    .collect();
                return Some(MenuEntry::Submenu { label: s(m, "label").unwrap_or_default(), icon: s(m, "icon"), children });
            }
            let action = s(m, "action")?;
            Some(MenuEntry::Item {
                label: s(m, "label").unwrap_or_else(|| action.clone()),
                action,
                icon: s(m, "icon"),
                tone: s(m, "tone"),
                disabled: on(m, "disabled"),
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Avatar {
    pub icon: Option<String>,
    pub text: Option<String>,
    pub color: Option<String>,
    pub circle: bool,
    pub plain: bool,
    pub svg: Option<String>,
}

impl Avatar {
    pub fn parse(raw: Option<&Value>) -> Option<Avatar> {
        match raw? {
            Value::String(icon) => Some(Avatar { icon: Some(icon.clone()), ..Avatar::default() }),
            Value::Object(m) => Some(Avatar {
                icon: s(m, "icon"),
                text: s(m, "text"),
                color: s(m, "color"),
                circle: m.get("shape").and_then(Value::as_str) == Some("circle"),
                plain: on(m, "plain"),
                svg: s(m, "svg"),
            }),
            _ => None,
        }
    }
}

