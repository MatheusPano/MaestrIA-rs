use crate::Rgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const WHITE: Self = Self::opaque(0xFFFFFF);

    pub const fn from_argb(v: u32) -> Self {
        Self {
            a: (v >> 24) as u8,
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
        }
    }

    pub const fn opaque(rgb: u32) -> Self {
        Self::from_argb(0xFF00_0000 | rgb)
    }

    pub fn to_argb(self) -> u32 {
        (u32::from(self.a) << 24)
            | (u32::from(self.r) << 16)
            | (u32::from(self.g) << 8)
            | u32::from(self.b)
    }

    pub fn rgb(self) -> Rgb {
        Rgb {
            r: self.r,
            g: self.g,
            b: self.b,
        }
    }

    pub fn with_alpha(self, alpha: f64) -> Self {
        Self {
            a: unit_to_byte(alpha),
            ..self
        }
    }

    pub fn alpha(self) -> f64 {
        f64::from(self.a) / 255.0
    }

    pub fn red(self) -> f64 {
        f64::from(self.r) / 255.0
    }

    pub fn green(self) -> f64 {
        f64::from(self.g) / 255.0
    }

    pub fn blue(self) -> f64 {
        f64::from(self.b) / 255.0
    }

    pub fn luminance(self) -> f64 {
        let lin = |c: f64| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.red()) + 0.7152 * lin(self.green()) + 0.0722 * lin(self.blue())
    }

    pub fn parse_hex(raw: &str) -> Option<Self> {
        let hex = raw.trim().replacen('#', "", 1);
        if (hex.len() != 6 && hex.len() != 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let value = u32::from_str_radix(&hex, 16).ok()?;
        Some(if hex.len() == 6 {
            Self::opaque(value)
        } else {
            Self::from_argb(value)
        })
    }

    pub fn to_hex(self) -> String {
        if self.a == 0xFF {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:08X}", self.to_argb())
        }
    }
}

impl From<Color> for Rgb {
    fn from(c: Color) -> Self {
        c.rgb()
    }
}

impl From<Rgb> for Color {
    fn from(c: Rgb) -> Self {
        Self {
            a: 0xFF,
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}

pub(crate) fn unit_to_byte(x: f64) -> u8 {
    (x * 255.0).round().clamp(0.0, 255.0) as u8
}
