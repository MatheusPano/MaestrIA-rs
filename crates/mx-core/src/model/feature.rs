use serde_json::{Value, json};

use crate::json::{Obj, get_bool, get_str};
use crate::model::Tint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FeatureOrHotfixKind {
    #[default]
    Feature,
    Hotfix,
}

impl FeatureOrHotfixKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Hotfix => "hotfix",
        }
    }

    pub fn by_name(name: Option<&str>) -> Self {
        match name {
            Some("hotfix") => Self::Hotfix,
            _ => Self::Feature,
        }
    }

    pub fn other(self) -> Self {
        match self {
            Self::Feature => Self::Hotfix,
            Self::Hotfix => Self::Feature,
        }
    }

    pub fn label(self) -> &'static str {
        self.name()
    }

    pub fn new_label(self) -> &'static str {
        self.pick("Nova feature", "Novo hotfix")
    }

    pub fn the(self) -> &'static str {
        self.pick("a feature", "o hotfix")
    }

    pub fn in_this(self) -> &'static str {
        self.pick("nessa feature", "nesse hotfix")
    }

    pub fn this_one(self) -> &'static str {
        self.pick("essa feature", "esse hotfix")
    }

    pub fn of_this(self) -> &'static str {
        self.pick("dessa feature", "desse hotfix")
    }

    pub fn of_the(self) -> &'static str {
        self.pick("da feature", "do hotfix")
    }

    pub fn dissolved(self) -> &'static str {
        self.pick("Feature dissolvida", "Hotfix dissolvido")
    }

    pub fn icon(self) -> &'static str {
        self.pick("track_changes", "bolt")
    }

    fn pick(self, feature: &'static str, hotfix: &'static str) -> &'static str {
        match self {
            Self::Feature => feature,
            Self::Hotfix => hotfix,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureOrHotfix {
    pub id: String,
    pub folder_root: String,
    pub name: String,
    pub brief: String,
    pub collapsed: bool,
    pub kind: FeatureOrHotfixKind,
    pub tint: Option<Tint>,
}

impl FeatureOrHotfix {
    pub fn new(
        id: impl Into<String>,
        folder_root: impl Into<String>,
        name: impl Into<String>,
        kind: FeatureOrHotfixKind,
    ) -> Self {
        FeatureOrHotfix {
            id: id.into(),
            folder_root: folder_root.into(),
            name: name.into(),
            brief: String::new(),
            collapsed: false,
            kind,
            tint: None,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("id".into(), json!(self.id));
        m.insert("folderRoot".into(), json!(self.folder_root));
        m.insert("name".into(), json!(self.name));
        m.insert("brief".into(), json!(self.brief));
        m.insert("collapsed".into(), json!(self.collapsed));
        m.insert("kind".into(), json!(self.kind.name()));
        if let Some(t) = self.tint {
            m.insert("tint".into(), json!(t.name()));
        }
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Option<FeatureOrHotfix> {
        let m = v.as_object()?;
        let id = get_str(m, "id").typed()??;
        let folder_root = get_str(m, "folderRoot").typed()??;
        let name = get_str(m, "name").typed()??;
        Some(FeatureOrHotfix {
            id: id.to_owned(),
            folder_root: folder_root.to_owned(),
            name: name.to_owned(),
            brief: get_str(m, "brief").typed()?.unwrap_or("").to_owned(),
            collapsed: get_bool(m, "collapsed").typed()?.unwrap_or(false),
            kind: FeatureOrHotfixKind::by_name(get_str(m, "kind").typed()?),
            tint: Tint::by_name(get_str(m, "tint").typed()?),
        })
    }
}
