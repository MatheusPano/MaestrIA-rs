mod dark_a;
mod dark_b;
mod light;

use std::sync::{Arc, LazyLock};

use crate::{Ansi, Color, Palette, Theme};

pub const DEFAULT_THEME_ID: &str = "maestria";

pub const RETIRED_ALIASES: [(&str, &str); 4] = [
    ("tokyo-night", "maestria"),
    ("catppuccin-frappe", "catppuccin-mocha"),
    ("rose-pine", "catppuccin-mocha"),
    ("chatgpt-dark", "graphite"),
];

pub(crate) struct ThemeSpec {
    id: &'static str,
    label: &'static str,
    dark: bool,
    canvas: u32,
    bg: u32,
    bg_sidebar: u32,
    bg_hover: u32,
    bg_active: u32,
    border: u32,
    fg: u32,
    fg_dim: u32,
    fg_faint: u32,
    accent: u32,
    green: u32,
    yellow: u32,
    red: u32,
    purple: u32,
    ansi: [u32; 8],
    bright: [Option<u32>; 8],
}

impl ThemeSpec {
    fn palette(&self) -> Palette {
        let c = Color::opaque;
        let [black, red, green, yellow, blue, magenta, cyan, white] = self.ansi.map(c);
        Palette {
            id: self.id.to_owned(),
            label: self.label.to_owned(),
            dark: self.dark,
            canvas: c(self.canvas),
            bg: c(self.bg),
            bg_sidebar: c(self.bg_sidebar),
            bg_hover: c(self.bg_hover),
            bg_active: c(self.bg_active),
            border: c(self.border),
            fg: c(self.fg),
            fg_dim: c(self.fg_dim),
            fg_faint: c(self.fg_faint),
            accent: c(self.accent),
            green: c(self.green),
            yellow: c(self.yellow),
            red: c(self.red),
            purple: c(self.purple),
            ansi: Ansi {
                black,
                red,
                green,
                yellow,
                blue,
                magenta,
                cyan,
                white,
                bright: self.bright.map(|b| b.map(c)),
            },
        }
    }
}

static BUILT_INS: LazyLock<Vec<Arc<Theme>>> = LazyLock::new(|| {
    dark_a::DARK_A
        .iter()
        .chain(dark_b::DARK_B.iter())
        .chain(light::LIGHT.iter())
        .map(|spec| Arc::new(Theme::new(spec.palette())))
        .collect()
});

pub fn built_ins() -> &'static [Arc<Theme>] {
    &BUILT_INS
}

pub fn built_in(id: &str) -> Option<&'static Arc<Theme>> {
    BUILT_INS.iter().find(|t| t.id == id)
}

pub fn default_theme() -> Arc<Theme> {
    BUILT_INS[0].clone()
}

pub fn retired_alias(id: &str) -> Option<&'static str> {
    RETIRED_ALIASES
        .iter()
        .find(|(from, _)| *from == id)
        .map(|(_, to)| *to)
}

pub fn is_built_in(id: &str) -> bool {
    built_in(id).is_some() || retired_alias(id).is_some()
}
