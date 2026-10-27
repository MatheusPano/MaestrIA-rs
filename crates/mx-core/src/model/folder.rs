use serde_json::{Value, json};

use crate::json::{Obj, get_bool, get_str};
use crate::model::Tint;

pub const LOOSE_NAME: &str = "Avulsos";

#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
    pub root: String,
    pub name: String,
    pub collapsed: bool,
    pub tint: Option<Tint>,
    pub is_loose: bool,
    pub is_repo: bool,
    pub branch: String,
}

impl Folder {
    pub fn new(root: impl Into<String>, name: impl Into<String>) -> Self {
        Folder {
            root: root.into(),
            name: name.into(),
            collapsed: false,
            tint: None,
            is_loose: false,
            is_repo: false,
            branch: String::new(),
        }
    }

    pub fn loose(home: Option<&str>) -> Self {
        Folder {
            is_loose: true,
            ..Folder::new(crate::paths::loose_root(home), LOOSE_NAME)
        }
    }

    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("root".into(), json!(self.root));
        m.insert("name".into(), json!(self.name));
        m.insert("collapsed".into(), json!(self.collapsed));
        if let Some(t) = self.tint {
            m.insert("tint".into(), json!(t.name()));
        }
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Option<Folder> {
        let m = v.as_object()?;
        let root = get_str(m, "root").typed()??;
        let name = get_str(m, "name").typed()??;
        let collapsed = get_bool(m, "collapsed").typed()?.unwrap_or(false);
        let tint = Tint::by_name(get_str(m, "tint").typed()?);
        Some(Folder {
            collapsed,
            tint,
            ..Folder::new(root, name)
        })
    }
}
