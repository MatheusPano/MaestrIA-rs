use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Face {
    pub family: &'static str,
    pub note: Option<&'static str>,
    pub bundled: bool,
}

const fn face(family: &'static str) -> Face {
    Face {
        family,
        note: None,
        bundled: false,
    }
}

const fn noted(family: &'static str, note: &'static str) -> Face {
    Face {
        family,
        note: Some(note),
        bundled: false,
    }
}

pub const HACK: Face = Face {
    family: "Hack",
    note: Some("vem no app"),
    bundled: true,
};

pub const FACES: [Face; 31] = [
    HACK,
    noted("Hack Nerd Font", "com os glifos do prompt"),
    face("JetBrainsMono Nerd Font"),
    face("FiraCode Nerd Font"),
    face("MesloLGS NF"),
    noted("SF Mono", "a do Terminal.app"),
    noted("Menlo", "a do macOS"),
    face("Monaco"),
    face("JetBrains Mono"),
    face("Fira Code"),
    face("IBM Plex Mono"),
    face("Source Code Pro"),
    face("Roboto Mono"),
    face("Inconsolata"),
    face("Cascadia Code"),
    face("Cascadia Mono"),
    face("Iosevka"),
    face("Berkeley Mono"),
    face("Geist Mono"),
    face("Commit Mono"),
    face("Maple Mono"),
    face("Monaspace Neon"),
    face("Victor Mono"),
    face("Anonymous Pro"),
    face("DejaVu Sans Mono"),
    face("Liberation Mono"),
    face("Noto Sans Mono"),
    face("Ubuntu Mono"),
    face("Andale Mono"),
    face("PT Mono"),
    face("Courier New"),
];

pub const PTY_FALLBACK: [&str; 12] = [
    "Menlo",
    "Monaco",
    "STIX Two Math",
    "Apple Symbols",
    "Noto Sans Symbols 2",
    "Noto Sans Symbols",
    "DejaVu Sans",
    "Noto Color Emoji",
    "Liberation Mono",
    "Courier New",
    "monospace",
    "sans-serif",
];

impl Face {
    pub fn available(&self, installed: &dyn Fn(&str) -> bool) -> bool {
        self.bundled || installed(self.family)
    }
}

pub fn installed_faces(installed: &dyn Fn(&str) -> bool) -> Vec<Face> {
    FACES
        .iter()
        .copied()
        .filter(|f| f.available(installed))
        .collect()
}

pub fn face_by_family(family: Option<&str>, installed: &dyn Fn(&str) -> bool) -> Face {
    FACES
        .iter()
        .copied()
        .find(|f| Some(f.family) == family && f.available(installed))
        .unwrap_or(HACK)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Typography {
    mono: String,
    size: f64,
    line: f64,
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

impl Typography {
    pub const MIN_SIZE: f64 = 9.0;
    pub const MAX_SIZE: f64 = 24.0;
    pub const MIN_LINE: f64 = 1.0;
    pub const MAX_LINE: f64 = 1.8;
    pub const SIZE_STEP: f64 = 1.0;
    pub const LINE_STEP: f64 = 0.1;
    pub const STANDARD_MONO: &str = "Hack";
    pub const STANDARD_SIZE: f64 = 13.0;
    pub const STANDARD_LINE: f64 = 1.2;

    pub fn standard() -> Self {
        Self {
            mono: Self::STANDARD_MONO.to_owned(),
            size: Self::STANDARD_SIZE,
            line: Self::STANDARD_LINE,
        }
    }

    pub fn new(
        mono: Option<&str>,
        size: Option<f64>,
        line: Option<f64>,
        installed: &dyn Fn(&str) -> bool,
    ) -> Self {
        Self {
            mono: face_by_family(Some(mono.unwrap_or(Self::STANDARD_MONO)), installed)
                .family
                .to_owned(),
            size: round2(
                size.unwrap_or(Self::STANDARD_SIZE)
                    .clamp(Self::MIN_SIZE, Self::MAX_SIZE),
            ),
            line: round2(
                line.unwrap_or(Self::STANDARD_LINE)
                    .clamp(Self::MIN_LINE, Self::MAX_LINE),
            ),
        }
    }

    pub fn mono(&self) -> &str {
        &self.mono
    }

    pub fn size(&self) -> f64 {
        self.size
    }

    pub fn line(&self) -> f64 {
        self.line
    }

    pub fn with(
        &self,
        mono: Option<&str>,
        size: Option<f64>,
        line: Option<f64>,
        installed: &dyn Fn(&str) -> bool,
    ) -> Self {
        Self::new(
            Some(mono.unwrap_or(&self.mono)),
            Some(size.unwrap_or(self.size)),
            Some(line.unwrap_or(self.line)),
            installed,
        )
    }

    pub fn clamp_zoom(&self, zoom: i64) -> i64 {
        let low = (Self::MIN_SIZE - self.size).ceil() as i64;
        let high = (Self::MAX_SIZE - self.size).floor() as i64;
        zoom.clamp(low, high)
    }

    pub fn size_at(&self, zoom: i64) -> f64 {
        self.size + self.clamp_zoom(zoom) as f64
    }

    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        if self.mono != Self::STANDARD_MONO {
            out.insert("mono".into(), Value::from(self.mono.clone()));
        }
        if self.size != Self::STANDARD_SIZE {
            out.insert("size".into(), Value::from(self.size));
        }
        if self.line != Self::STANDARD_LINE {
            out.insert("line".into(), Value::from(self.line));
        }
        Value::Object(out)
    }

    pub fn from_json(json: Option<&Value>, installed: &dyn Fn(&str) -> bool) -> Self {
        let Some(j) = json.and_then(Value::as_object) else {
            return Self::standard();
        };
        Self::new(
            j.get("mono").and_then(Value::as_str),
            j.get("size").and_then(Value::as_f64),
            j.get("line").and_then(Value::as_f64),
            installed,
        )
    }

    pub fn style_key(&self, zoom: i64) -> StyleKey {
        StyleKey {
            mono: self.mono.clone(),
            size_bits: self.size_at(zoom).to_bits(),
            line_bits: self.line.to_bits(),
        }
    }
}

impl Default for Typography {
    fn default() -> Self {
        Self::standard()
    }
}

impl fmt::Display for Typography {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:?}px/{:?}", self.mono, self.size, self.line)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StyleKey {
    pub mono: String,
    size_bits: u64,
    line_bits: u64,
}

impl StyleKey {
    pub fn size(&self) -> f64 {
        f64::from_bits(self.size_bits)
    }

    pub fn line(&self) -> f64 {
        f64::from_bits(self.line_bits)
    }
}

#[derive(Debug)]
pub struct StyleCache<T> {
    styles: HashMap<StyleKey, Arc<T>>,
}

impl<T> Default for StyleCache<T> {
    fn default() -> Self {
        Self {
            styles: HashMap::new(),
        }
    }
}

impl<T> StyleCache<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_or_insert_with(
        &mut self,
        key: StyleKey,
        make: impl FnOnce(&StyleKey) -> T,
    ) -> Arc<T> {
        if let Some(style) = self.styles.get(&key) {
            return style.clone();
        }
        let style = Arc::new(make(&key));
        self.styles.insert(key, style.clone());
        style
    }

    pub fn len(&self) -> usize {
        self.styles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}
