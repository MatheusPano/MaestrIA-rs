use std::f64::consts::PI;

use crate::Color;
use crate::color::unit_to_byte;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    pub hue: f64,
}

pub(crate) fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f64) -> f64 {
    let x = v.clamp(0.0, 1.0);
    if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

fn cbrt(v: f64) -> f64 {
    v.powf(1.0 / 3.0)
}

impl Oklch {
    pub fn new(l: f64, c: f64, hue: f64) -> Self {
        Self { l, c, hue }
    }

    pub fn from_color(color: Color) -> Self {
        let r = srgb_to_linear(color.red());
        let g = srgb_to_linear(color.green());
        let b = srgb_to_linear(color.blue());
        let l = cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
        let m = cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
        let s = cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
        let ok_a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
        let ok_b = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
        Self {
            l: 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            c: (ok_a * ok_a + ok_b * ok_b).sqrt(),
            hue: ok_b.atan2(ok_a) * 180.0 / PI,
        }
    }

    pub fn to_srgb_unit(self) -> [f64; 3] {
        let h = self.hue * PI / 180.0;
        let ok_a = self.c * h.cos();
        let ok_b = self.c * h.sin();
        let l = (self.l + 0.3963377774 * ok_a + 0.2158037573 * ok_b).powf(3.0);
        let m = (self.l - 0.1055613458 * ok_a - 0.0638541728 * ok_b).powf(3.0);
        let s = (self.l - 0.0894841775 * ok_a - 1.2914855480 * ok_b).powf(3.0);
        [
            linear_to_srgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
            linear_to_srgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
            linear_to_srgb(-0.0041960771 * l - 0.7034186147 * m + 1.7076069010 * s),
        ]
    }

    pub fn to_color(self) -> Color {
        let [r, g, b] = self.to_srgb_unit();
        Color {
            a: 0xFF,
            r: unit_to_byte(r),
            g: unit_to_byte(g),
            b: unit_to_byte(b),
        }
    }
}
