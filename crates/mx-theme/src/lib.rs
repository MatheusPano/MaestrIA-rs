#![forbid(unsafe_code)]

mod builtin;
mod color;
mod json;
mod oklch;
mod palette;
mod registry;
mod terminal;
mod tint;
mod typography;

pub use builtin::{
    DEFAULT_THEME_ID, RETIRED_ALIASES, built_in, built_ins, default_theme, is_built_in,
    retired_alias,
};
pub use color::Color;
pub use json::ThemeError;
pub use oklch::Oklch;
pub use palette::{Ansi, CLAUDE, Palette, Theme};
pub use registry::Themes;
pub use terminal::{Rgb, TerminalPalette};
pub use tint::Tint;
pub use typography::{
    FACES, Face, HACK, PTY_FALLBACK, StyleCache, StyleKey, Typography, face_by_family,
    installed_faces,
};
