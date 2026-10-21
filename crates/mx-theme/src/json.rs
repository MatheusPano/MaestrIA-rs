use serde_json::{Map, Value};

use crate::{Ansi, Color, Palette};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ThemeError {
    #[error("{0}")]
    Syntax(String),
    #[error("não é um objeto")]
    NotAnObject,
    #[error("\"id\" é obrigatório")]
    MissingId,
    #[error("\"label\" é obrigatório")]
    MissingLabel,
    #[error("\"ansi\" é obrigatório")]
    MissingAnsi,
    #[error("\"{0}\" precisa ser uma cor #RRGGBB")]
    BadColor(String),
    #[error("\"{0}\" é o id de um tema embutido")]
    BuiltInId(String),
    #[error("outro plugin já tem um tema \"{0}\"")]
    DuplicateId(String),
}

const BRIGHT_KEYS: [&str; 8] = [
    "brightBlack",
    "brightRed",
    "brightGreen",
    "brightYellow",
    "brightBlue",
    "brightMagenta",
    "brightCyan",
    "brightWhite",
];

fn maybe_color(from: &Map<String, Value>, key: &str) -> Option<Color> {
    from.get(key)
        .and_then(Value::as_str)
        .and_then(Color::parse_hex)
}

fn color(from: &Map<String, Value>, key: &str, prefix: &str) -> Result<Color, ThemeError> {
    maybe_color(from, key).ok_or_else(|| ThemeError::BadColor(format!("{prefix}{key}")))
}

fn required_text(j: &Map<String, Value>, key: &str) -> Option<String> {
    j.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

impl Palette {
    pub fn from_json_str(text: &str) -> Result<Palette, ThemeError> {
        let value: Value =
            serde_json::from_str(text).map_err(|e| ThemeError::Syntax(e.to_string()))?;
        Self::from_json(&value)
    }

    pub fn from_json(value: &Value) -> Result<Palette, ThemeError> {
        let j = value.as_object().ok_or(ThemeError::NotAnObject)?;
        let id = required_text(j, "id").ok_or(ThemeError::MissingId)?;
        let label = required_text(j, "label").ok_or(ThemeError::MissingLabel)?;
        let ansi = j
            .get("ansi")
            .and_then(Value::as_object)
            .ok_or(ThemeError::MissingAnsi)?;
        let ui = |key: &str| color(j, key, "");
        let a = |key: &str| color(ansi, key, "ansi.");
        Ok(Palette {
            id,
            label,
            dark: j.get("dark") != Some(&Value::Bool(false)),
            canvas: ui("canvas")?,
            bg: ui("bg")?,
            bg_sidebar: ui("bgSidebar")?,
            bg_hover: ui("bgHover")?,
            bg_active: ui("bgActive")?,
            border: ui("border")?,
            fg: ui("fg")?,
            fg_dim: ui("fgDim")?,
            fg_faint: ui("fgFaint")?,
            accent: ui("accent")?,
            green: ui("green")?,
            yellow: ui("yellow")?,
            red: ui("red")?,
            purple: ui("purple")?,
            ansi: Ansi {
                black: a("black")?,
                red: a("red")?,
                green: a("green")?,
                yellow: a("yellow")?,
                blue: a("blue")?,
                magenta: a("magenta")?,
                cyan: a("cyan")?,
                white: a("white")?,
                bright: BRIGHT_KEYS.map(|k| maybe_color(ansi, k)),
            },
        })
    }
}
