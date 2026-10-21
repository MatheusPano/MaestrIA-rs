use super::{Color, FontStyle, TextBaseline, TextDecorationStyle, TextLeadingDistribution, TextOverflow, color};
use crate::tree::Node;
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);

pub fn font_weight(v: &Value) -> Option<FontWeight> {
    if let Some(n) = v.as_int()
        && (100..=900).contains(&n)
        && n % 100 == 0
    {
        return Some(FontWeight(n as u16));
    }
    let w = match v.as_str()? {
        "w100" => 100,
        "w200" => 200,
        "w300" => 300,
        "w400" | "normal" => 400,
        "w500" => 500,
        "w600" => 600,
        "w700" | "bold" => 700,
        "w800" => 800,
        "w900" => 900,
        _ => return None,
    };
    Some(FontWeight(w))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextDecoration {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
}

pub fn text_decoration(v: &Value) -> TextDecoration {
    if let Some(list) = v.as_list() {
        return list.iter().map(text_decoration).fold(TextDecoration::default(), |a, b| TextDecoration {
            underline: a.underline || b.underline,
            overline: a.overline || b.overline,
            line_through: a.line_through || b.line_through,
        });
    }
    let mut d = TextDecoration::default();
    match v.as_str() {
        Some("lineThrough") => d.line_through = true,
        Some("overline") => d.overline = true,
        Some("underline") => d.underline = true,
        _ => {}
    }
    d
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyle {
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub font_size: Option<f64>,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub letter_spacing: Option<f64>,
    pub word_spacing: Option<f64>,
    pub text_baseline: Option<TextBaseline>,
    pub height: Option<f64>,
    pub leading_distribution: Option<TextLeadingDistribution>,
    pub decoration: TextDecoration,
    pub decoration_color: Option<Color>,
    pub decoration_style: Option<TextDecorationStyle>,
    pub decoration_thickness: Option<f64>,
    pub font_family: Option<String>,
    pub font_family_fallback: Option<Vec<String>>,
    pub overflow: Option<TextOverflow>,
}

pub fn text_style(node: &Node, key: &str) -> Option<TextStyle> {
    let v = node.arg(key);
    v.as_map()?;
    let d = |k: &str| v.get(k).as_double();
    Some(TextStyle {
        color: color(v.get("color")),
        background_color: color(v.get("backgroundColor")),
        font_size: d("fontSize"),
        font_weight: font_weight(v.get("fontWeight")),
        font_style: FontStyle::decode(v.get("fontStyle")),
        letter_spacing: d("letterSpacing"),
        word_spacing: d("wordSpacing"),
        text_baseline: TextBaseline::decode(node.arg("textBaseline")),
        height: d("height"),
        leading_distribution: TextLeadingDistribution::decode(v.get("leadingDistribution")),
        decoration: text_decoration(v.get("decoration")),
        decoration_color: color(v.get("decorationColor")),
        decoration_style: TextDecorationStyle::decode(v.get("decorationStyle")),
        decoration_thickness: d("decorationThickness"),
        font_family: v.get("fontFamily").as_str().map(str::to_owned),
        font_family_fallback: v
            .get("fontFamilyFallback")
            .as_list()
            .map(|l| l.iter().map(|s| s.as_str().unwrap_or("").to_owned()).collect()),
        overflow: TextOverflow::decode(node.arg("overflow")),
    })
}
