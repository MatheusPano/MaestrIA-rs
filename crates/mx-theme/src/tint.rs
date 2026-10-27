use crate::oklch::Oklch;
use crate::{Ansi, Color};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tint {
    Red,
    Orange,
    Yellow,
    Lime,
    Green,
    Aqua,
    Cyan,
    Petrol,
    Violet,
    Magenta,
    Pink,
    Brown,
    Grey,
}

impl Tint {
    pub const COUNT: usize = 13;

    pub const ALL: [Tint; Self::COUNT] = [
        Tint::Red,
        Tint::Orange,
        Tint::Yellow,
        Tint::Lime,
        Tint::Green,
        Tint::Aqua,
        Tint::Cyan,
        Tint::Petrol,
        Tint::Violet,
        Tint::Magenta,
        Tint::Pink,
        Tint::Brown,
        Tint::Grey,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tint::Red => "red",
            Tint::Orange => "orange",
            Tint::Yellow => "yellow",
            Tint::Lime => "lime",
            Tint::Green => "green",
            Tint::Aqua => "aqua",
            Tint::Cyan => "cyan",
            Tint::Petrol => "petrol",
            Tint::Violet => "violet",
            Tint::Magenta => "magenta",
            Tint::Pink => "pink",
            Tint::Brown => "brown",
            Tint::Grey => "grey",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tint::Red => "Vermelho",
            Tint::Orange => "Laranja",
            Tint::Yellow => "Amarelo",
            Tint::Lime => "Lima",
            Tint::Green => "Verde",
            Tint::Aqua => "Verde-água",
            Tint::Cyan => "Ciano",
            Tint::Petrol => "Azul-petróleo",
            Tint::Violet => "Violeta",
            Tint::Magenta => "Magenta",
            Tint::Pink => "Rosa",
            Tint::Brown => "Marrom",
            Tint::Grey => "Cinza",
        }
    }

    pub fn hue(self) -> f64 {
        match self {
            Tint::Red => 25.0,
            Tint::Orange | Tint::Brown => 55.0,
            Tint::Yellow => 95.0,
            Tint::Lime => 125.0,
            Tint::Green => 150.0,
            Tint::Aqua => 175.0,
            Tint::Cyan => 205.0,
            Tint::Petrol => 235.0,
            Tint::Violet => 290.0,
            Tint::Magenta => 330.0,
            Tint::Pink | Tint::Grey => 0.0,
        }
    }

    pub fn by_name(name: Option<&str>) -> Option<Tint> {
        let name = name?;
        Self::ALL.into_iter().find(|t| t.name() == name)
    }
}

fn running_mean(values: impl Iterator<Item = f64>) -> f64 {
    let mut result = 0.0;
    let mut count = 0.0;
    for v in values {
        count += 1.0;
        result += (v - result) / count;
    }
    result
}

pub(crate) fn compute_tints(ansi: &Ansi) -> [Color; Tint::COUNT] {
    let base = [
        ansi.red,
        ansi.yellow,
        ansi.bright_green(),
        ansi.cyan,
        ansi.magenta,
    ]
    .map(Oklch::from_color);
    let lightness = running_mean(base.iter().map(|c| c.l));
    let chroma = running_mean(base.iter().map(|c| c.c));
    Tint::ALL.map(|tint| match tint {
        Tint::Grey => ansi.bright_black(),
        Tint::Brown => Oklch::new(lightness * 0.8, chroma * 0.6, tint.hue()).to_color(),
        _ => Oklch::new(lightness, chroma, tint.hue()).to_color(),
    })
}
