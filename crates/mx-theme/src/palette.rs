use std::ops::Deref;

use crate::tint::{Tint, compute_tints};
use crate::{Color, Rgb, TerminalPalette};

pub const CLAUDE: Color = Color::opaque(0xD97757);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ansi {
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub white: Color,
    pub bright: [Option<Color>; 8],
}

impl Ansi {
    pub fn normal(&self) -> [Color; 8] {
        [
            self.black,
            self.red,
            self.green,
            self.yellow,
            self.blue,
            self.magenta,
            self.cyan,
            self.white,
        ]
    }

    pub fn bright(&self, index: usize) -> Color {
        self.bright[index].unwrap_or(self.normal()[index])
    }

    pub fn bright_black(&self) -> Color {
        self.bright(0)
    }

    pub fn bright_red(&self) -> Color {
        self.bright(1)
    }

    pub fn bright_green(&self) -> Color {
        self.bright(2)
    }

    pub fn bright_yellow(&self) -> Color {
        self.bright(3)
    }

    pub fn bright_blue(&self) -> Color {
        self.bright(4)
    }

    pub fn bright_magenta(&self) -> Color {
        self.bright(5)
    }

    pub fn bright_cyan(&self) -> Color {
        self.bright(6)
    }

    pub fn bright_white(&self) -> Color {
        self.bright(7)
    }

    pub fn all16(&self) -> [Color; 16] {
        let normal = self.normal();
        std::array::from_fn(|i| if i < 8 { normal[i] } else { self.bright(i - 8) })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub id: String,
    pub label: String,
    pub dark: bool,
    pub canvas: Color,
    pub bg: Color,
    pub bg_sidebar: Color,
    pub bg_hover: Color,
    pub bg_active: Color,
    pub border: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub fg_faint: Color,
    pub accent: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub purple: Color,
    pub ansi: Ansi,
}

impl Palette {
    pub fn shadow(&self) -> Color {
        Color::from_argb(if self.dark { 0x5900_0000 } else { 0x1410_1828 })
    }

    pub fn on_accent(&self) -> Color {
        if self.accent.luminance() > 0.5 {
            self.canvas
        } else {
            Color::WHITE
        }
    }

    pub fn group_tints(&self) -> [Color; 4] {
        [
            self.ansi.cyan,
            self.ansi.magenta,
            self.ansi.blue,
            self.ansi.bright_green(),
        ]
    }

    pub fn group_tint(&self, id: &str) -> Color {
        let sum: u64 = id.encode_utf16().map(u64::from).sum();
        self.group_tints()[(sum % 4) as usize]
    }

    pub fn terminal(&self) -> TerminalPalette {
        let ansi: [Rgb; 16] = self.ansi.all16().map(Color::rgb);
        TerminalPalette {
            foreground: self.fg.rgb(),
            background: self.bg.rgb(),
            cursor: self.accent.rgb(),
            ansi,
            selection: self.accent.with_alpha(0.3),
            search_hit_background: self.yellow.rgb(),
            search_hit_background_current: self.accent.rgb(),
            search_hit_foreground: self.canvas.rgb(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    palette: Palette,
    tints: [Color; Tint::COUNT],
}

impl Theme {
    pub fn new(palette: Palette) -> Self {
        let tints = compute_tints(&palette.ansi);
        Self { palette, tints }
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn tint(&self, tint: Tint) -> Color {
        self.tints[tint as usize]
    }

    pub fn tints(&self) -> &[Color; Tint::COUNT] {
        &self.tints
    }
}

impl Deref for Theme {
    type Target = Palette;

    fn deref(&self) -> &Palette {
        &self.palette
    }
}

impl From<Palette> for Theme {
    fn from(p: Palette) -> Self {
        Self::new(p)
    }
}
