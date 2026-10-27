use gpui::{Font, FontWeight, Hsla, UnderlineStyle, font, px};

use super::syntax::Tone;
use crate::chrome::style::MONO;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

pub const SETUP_PAD: Pad = Pad { left: 18., top: 14., right: 18., bottom: 24. };

#[derive(Clone, Debug, PartialEq)]
pub struct Gutter {
    pub color: Hsla,
    pub current: Hsla,
    pub font_size: f32,
    pub fold: Option<Hsla>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxColors {
    pub plain: Hsla,
    pub comment: Hsla,
    pub keyword: Hsla,
    pub kind: Hsla,
    pub function: Hsla,
    pub string: Hsla,
    pub number: Hsla,
    pub attr: Hsla,
    pub variable: Hsla,
    pub meta: Hsla,
}

impl SyntaxColors {
    pub fn of(&self, tone: Tone) -> Hsla {
        match tone {
            Tone::Comment => self.comment,
            Tone::Keyword => self.keyword,
            Tone::Type => self.kind,
            Tone::Function | Tone::Section | Tone::Link => self.function,
            Tone::String | Tone::Added => self.string,
            Tone::Number | Tone::Literal | Tone::Bullet => self.number,
            Tone::Key => self.attr,
            Tone::Variable | Tone::Removed => self.variable,
            Tone::Meta => self.meta,
            Tone::Emphasis | Tone::Strong => self.plain,
        }
    }
}

pub fn tone_font(tone: Option<Tone>) -> Font {
    let base = font(MONO);
    match tone {
        Some(Tone::Comment | Tone::Emphasis) => base.italic(),
        Some(Tone::Strong) => base.bold(),
        Some(Tone::Section) => Font { weight: FontWeight::SEMIBOLD, ..base },
        _ => base,
    }
}

pub fn tone_underline(tone: Option<Tone>, color: Hsla) -> Option<UnderlineStyle> {
    (tone == Some(Tone::Link)).then_some(UnderlineStyle { thickness: px(1.), color: Some(color), wavy: false })
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub fg: Hsla,
    pub caret: Hsla,
    pub selection: Hsla,
    pub thumb: Hsla,
    pub wrap: bool,
    pub code: bool,
    pub language: String,
    pub syntax: Option<SyntaxColors>,
    pub gutter: Option<Gutter>,
    pub found: Hsla,
    pub found_current: Hsla,
    pub current_line: Option<Hsla>,
    pub hint: Option<(String, Hsla)>,
    pub pad: Pad,
}

impl Default for EditorStyle {
    fn default() -> Self {
        Self {
            font_size: 12.5,
            line_height: 1.55,
            fg: gpui::white(),
            caret: gpui::white(),
            selection: gpui::blue().opacity(0.3),
            thumb: gpui::white().opacity(0.2),
            wrap: true,
            code: false,
            language: String::new(),
            syntax: None,
            gutter: None,
            found: gpui::yellow().opacity(0.25),
            found_current: gpui::yellow().opacity(0.5),
            current_line: None,
            hint: None,
            pad: SETUP_PAD,
        }
    }
}
