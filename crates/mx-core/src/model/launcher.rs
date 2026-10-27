use serde_json::{Value, json};

use crate::json::{Obj, get_str};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LauncherIcon {
    #[default]
    Terminal,
    Monitor,
    Chart,
    Git,
    Server,
    Database,
    Rocket,
    Bug,
    Edit,
    Box,
}

impl LauncherIcon {
    pub const ALL: [LauncherIcon; 10] = [
        Self::Terminal,
        Self::Monitor,
        Self::Chart,
        Self::Git,
        Self::Server,
        Self::Database,
        Self::Rocket,
        Self::Bug,
        Self::Edit,
        Self::Box,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Monitor => "monitor",
            Self::Chart => "chart",
            Self::Git => "git",
            Self::Server => "server",
            Self::Database => "database",
            Self::Rocket => "rocket",
            Self::Bug => "bug",
            Self::Edit => "edit",
            Self::Box => "box",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Monitor => "Monitor",
            Self::Chart => "Gráfico",
            Self::Git => "Git",
            Self::Server => "Servidor",
            Self::Database => "Banco",
            Self::Rocket => "Foguete",
            Self::Bug => "Bug",
            Self::Edit => "Editor",
            Self::Box => "Caixa",
        }
    }

    pub fn by_name(name: Option<&str>) -> Self {
        Self::ALL
            .into_iter()
            .find(|i| Some(i.name()) == name)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Launcher {
    pub id: String,
    pub name: String,
    pub command: String,
    pub icon: LauncherIcon,
}

impl Launcher {
    pub fn tint_index(&self, palette_len: usize) -> usize {
        crate::model::group::tint_index_of(&self.id, palette_len)
    }

    pub fn to_json(&self) -> Value {
        let mut m = Obj::new();
        m.insert("id".into(), json!(self.id));
        m.insert("name".into(), json!(self.name));
        m.insert("command".into(), json!(self.command));
        m.insert("icon".into(), json!(self.icon.name()));
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Option<Launcher> {
        let m = v.as_object()?;
        let id = get_str(m, "id").typed()?.unwrap_or("").to_owned();
        let name = get_str(m, "name").typed()?.unwrap_or("").trim().to_owned();
        let command = get_str(m, "command")
            .typed()?
            .unwrap_or("")
            .trim()
            .to_owned();
        if id.is_empty() || name.is_empty() || command.is_empty() {
            return None;
        }
        let icon = LauncherIcon::by_name(get_str(m, "icon").typed()?);
        Some(Launcher {
            id,
            name,
            command,
            icon,
        })
    }
}
