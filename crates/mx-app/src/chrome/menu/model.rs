use gpui::SharedString;
use mx_theme::{Color, Theme, Tint};

use crate::chrome::icon::Icon;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
    Dim,
    Faint,
    Accent,
    Red,
    Yellow,
    Green,
    Purple,
    Tint(Tint),
    Group(usize),
}

impl Tone {
    pub fn color(self, t: &Theme) -> Color {
        match self {
            Tone::Dim => t.fg_dim,
            Tone::Faint => t.fg_faint,
            Tone::Accent => t.accent,
            Tone::Red => t.red,
            Tone::Yellow => t.yellow,
            Tone::Green => t.green,
            Tone::Purple => t.purple,
            Tone::Tint(tint) => t.tint(tint),
            Tone::Group(i) => t.group_tints()[i % 4],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Glyph {
    Icon(Icon, Tone, f32),
    Claude,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trailing {
    Chip(SharedString),
    Ghost(usize),
    Chevron,
    Icon(Icon),
    Kind(Icon),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem<T> {
    pub label: SharedString,
    pub glyph: Option<Glyph>,
    pub subtitle: Option<(SharedString, Tone)>,
    pub chord: Option<SharedString>,
    pub trailing: Vec<Trailing>,
    pub tone: Option<Tone>,
    pub disabled: bool,
    pub keep_open: bool,
    pub indent: f32,
    pub value: Option<T>,
}

impl<T> MenuItem<T> {
    pub fn new(label: impl Into<SharedString>, value: T) -> Self {
        Self {
            label: label.into(),
            glyph: None,
            subtitle: None,
            chord: None,
            trailing: Vec::new(),
            tone: None,
            disabled: false,
            keep_open: false,
            indent: 0.,
            value: Some(value),
        }
    }

    pub fn icon(mut self, icon: Icon, tone: Tone) -> Self {
        self.glyph = Some(Glyph::Icon(icon, tone, 14.));
        self
    }

    pub fn dot(mut self, icon: Icon, tone: Tone, size: f32) -> Self {
        self.glyph = Some(Glyph::Icon(icon, tone, size));
        self
    }

    pub fn glyph(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub fn subtitle(mut self, text: impl Into<SharedString>, tone: Tone) -> Self {
        self.subtitle = Some((text.into(), tone));
        self
    }

    pub fn chord(mut self, chord: Option<impl Into<SharedString>>) -> Self {
        self.chord = chord.map(Into::into);
        self
    }

    pub fn trailing(mut self, t: Trailing) -> Self {
        self.trailing.push(t);
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    pub fn danger(self) -> Self {
        self.tone(Tone::Red)
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn keep_open(mut self) -> Self {
        self.keep_open = true;
        self
    }

    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }

    pub fn height(&self) -> f32 {
        if self.subtitle.is_some() { 48. } else { 36. }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry<T> {
    Item(MenuItem<T>),
    Divider,
    Caption(SharedString),
    Submenu(MenuItem<T>, Vec<MenuEntry<T>>),
}

impl<T> MenuEntry<T> {
    pub fn height(&self) -> f32 {
        match self {
            MenuEntry::Item(i) | MenuEntry::Submenu(i, _) => i.height(),
            MenuEntry::Divider => 9.,
            MenuEntry::Caption(_) => 26.,
        }
    }

    pub fn label(&self) -> Option<&str> {
        match self {
            MenuEntry::Item(i) | MenuEntry::Submenu(i, _) => Some(&i.label),
            _ => None,
        }
    }
}

impl<T> From<MenuItem<T>> for MenuEntry<T> {
    fn from(item: MenuItem<T>) -> Self {
        MenuEntry::Item(item)
    }
}

#[cfg(test)]
pub fn labels<T>(entries: &[MenuEntry<T>]) -> Vec<String> {
    entries
        .iter()
        .map(|e| match e {
            MenuEntry::Divider => "—".to_owned(),
            MenuEntry::Caption(c) => format!("[{c}]"),
            other => other.label().unwrap_or_default().to_owned(),
        })
        .collect()
}

#[cfg(test)]
pub fn find<'a, T>(entries: &'a [MenuEntry<T>], label: &str) -> Option<&'a MenuEntry<T>> {
    entries.iter().find(|e| e.label() == Some(label))
}

pub fn content_height<T>(entries: &[MenuEntry<T>]) -> f32 {
    entries.iter().map(MenuEntry::height).sum::<f32>() + 10.
}
