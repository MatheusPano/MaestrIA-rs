#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub const ALL: [Tint; 13] = [
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

    pub fn by_name(name: Option<&str>) -> Option<Tint> {
        let name = name?;
        Tint::ALL.into_iter().find(|t| t.name() == name)
    }
}
