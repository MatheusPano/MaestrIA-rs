use gpui::{
    Font, FontFeatures, FontStyle, FontWeight, Hsla, Rgba, StrikethroughStyle, TextRun,
    UnderlineStyle, px,
};
use mx_claude::markdown::MdAlert;
use mx_theme::{Color, Theme};

pub const SANS: &str = "Inter";
pub const MONO: &str = "Hack";
pub const BASE_SIZE: f32 = 13.5;

pub fn hsla(color: Color) -> Hsla {
    Rgba {
        r: color.red() as f32,
        g: color.green() as f32,
        b: color.blue() as f32,
        a: color.alpha() as f32,
    }
    .into()
}

#[derive(Clone)]
pub struct Sheet {
    pub size: f32,
    pub fg: Hsla,
    pub fg_dim: Hsla,
    pub fg_faint: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub link_line: Hsla,
    pub quote_bar: Hsla,
    pub bg: Hsla,
    pub bg_hover: Hsla,
    pub bg_active: Hsla,
    pub bg_sidebar: Hsla,
    pub border: Hsla,
    note: Hsla,
    tip: Hsla,
    important: Hsla,
    warning: Hsla,
    caution: Hsla,
}

impl Sheet {
    pub fn new(theme: &Theme, zoom: f32) -> Self {
        Self {
            size: BASE_SIZE + zoom,
            fg: hsla(theme.fg),
            fg_dim: hsla(theme.fg_dim),
            fg_faint: hsla(theme.fg_faint),
            accent: hsla(theme.accent),
            on_accent: hsla(theme.on_accent()),
            link_line: hsla(theme.accent.with_alpha(0.45)),
            quote_bar: hsla(theme.accent.with_alpha(0.55)),
            bg: hsla(theme.bg),
            bg_hover: hsla(theme.bg_hover),
            bg_active: hsla(theme.bg_active),
            bg_sidebar: hsla(theme.bg_sidebar),
            border: hsla(theme.border),
            note: hsla(theme.ansi.blue),
            tip: hsla(theme.green),
            important: hsla(theme.purple),
            warning: hsla(theme.yellow),
            caution: hsla(theme.red),
        }
    }

    pub fn mono_size(&self) -> f32 {
        self.size - 1.7
    }

    pub fn table_size(&self) -> f32 {
        self.size - 1.
    }

    pub fn heading(&self, level: u8) -> (f32, Hsla, f32, f32) {
        let s = self.size;
        match level {
            1 => (s + 7.5, self.fg, 6., 6.),
            2 => (s + 3., self.fg, 16., 4.),
            3 => (s + 1., self.fg_dim, 12., 2.),
            4 => (s, self.fg_dim, 0., 0.),
            5 => (s - 0.5, self.fg_dim, 0., 0.),
            _ => (s - 1., self.fg_faint, 0., 0.),
        }
    }

    pub fn alert(&self, kind: MdAlert) -> (Hsla, &'static str) {
        match kind {
            MdAlert::Note => (self.note, "Note"),
            MdAlert::Tip => (self.tip, "Tip"),
            MdAlert::Important => (self.important, "Important"),
            MdAlert::Warning => (self.warning, "Warning"),
            MdAlert::Caution => (self.caution, "Caution"),
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: bool,
    pub faint: bool,
}

pub fn font(family: &str, bold: bool, italic: bool) -> Font {
    Font {
        family: family.to_owned().into(),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: if bold {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        },
        style: if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
    }
}

pub fn run(len: usize, marks: Marks, base: Hsla, sheet: &Sheet) -> TextRun {
    let family = if marks.code { MONO } else { SANS };
    let color = if marks.link {
        sheet.accent
    } else if marks.strike || marks.faint {
        sheet.fg_faint
    } else {
        base
    };
    TextRun {
        len,
        font: font(family, marks.bold, marks.italic),
        color,
        background_color: marks.code.then_some(sheet.bg_active),
        underline: marks.link.then_some(UnderlineStyle {
            thickness: px(1.),
            color: Some(sheet.link_line),
            wavy: false,
        }),
        strikethrough: marks.strike.then_some(StrikethroughStyle {
            thickness: px(1.),
            color: Some(sheet.fg_faint),
        }),
    }
}
