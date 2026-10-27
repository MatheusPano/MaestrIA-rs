use crate::Color;
use crate::builtin::default_theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn hex(v: u32) -> Self {
        Self {
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
        }
    }

    pub fn to_u32(self) -> u32 {
        (u32::from(self.r) << 16) | (u32::from(self.g) << 8) | u32::from(self.b)
    }

    pub fn scaled(self, factor: f32) -> Self {
        let s = |c: u8| (f32::from(c) * factor).round().clamp(0.0, 255.0) as u8;
        Self {
            r: s(self.r),
            g: s(self.g),
            b: s(self.b),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalPalette {
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub ansi: [Rgb; 16],
    pub selection: Color,
    pub search_hit_background: Rgb,
    pub search_hit_background_current: Rgb,
    pub search_hit_foreground: Rgb,
}

impl TerminalPalette {
    pub fn maestria_dark() -> Self {
        default_theme().terminal()
    }

    pub fn indexed(&self, index: u8) -> Rgb {
        match index {
            0..=15 => self.ansi[usize::from(index)],
            16..=231 => {
                let i = index - 16;
                let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                Rgb {
                    r: level(i / 36),
                    g: level((i / 6) % 6),
                    b: level(i % 6),
                }
            }
            232..=255 => {
                let v = 8 + (index - 232) * 10;
                Rgb { r: v, g: v, b: v }
            }
        }
    }
}

impl Default for TerminalPalette {
    fn default() -> Self {
        Self::maestria_dark()
    }
}
