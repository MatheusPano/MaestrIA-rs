use super::{BorderStyle, BoxShape, Color, color};
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius {
    pub x: f64,
    pub y: f64,
}

pub fn radius(v: &Value) -> Option<Radius> {
    let x = v.get("x").as_double()?;
    Some(Radius { x, y: v.get("y").as_double().unwrap_or(x) })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderRadius {
    pub top_start: Radius,
    pub top_end: Radius,
    pub bottom_start: Radius,
    pub bottom_end: Radius,
}

pub fn border_radius(v: &Value) -> Option<BorderRadius> {
    let a = radius(v.at(0))?;
    let b = radius(v.at(1));
    let c = radius(v.at(2));
    let d = radius(v.at(3));
    Some(BorderRadius {
        top_start: a,
        top_end: b.unwrap_or(a),
        bottom_start: c.unwrap_or(a),
        bottom_end: d.or(b).unwrap_or(a),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderSide {
    pub color: Color,
    pub width: f64,
    pub style: BorderStyle,
}

pub fn border_side(v: &Value) -> Option<BorderSide> {
    v.as_map()?;
    Some(BorderSide {
        color: color(v.get("color")).unwrap_or(Color::BLACK),
        width: v.get("width").as_double().unwrap_or(1.0),
        style: BorderStyle::decode(v.get("style")).unwrap_or(BorderStyle::Solid),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub start: BorderSide,
    pub top: BorderSide,
    pub end: BorderSide,
    pub bottom: BorderSide,
}

pub fn border(v: &Value) -> Option<Border> {
    let a = border_side(v.at(0))?;
    let b = border_side(v.at(1));
    let c = border_side(v.at(2));
    let d = border_side(v.at(3));
    Some(Border { start: a, top: b.unwrap_or(a), end: c.unwrap_or(a), bottom: d.or(b).unwrap_or(a) })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offset {
    pub x: f64,
    pub y: f64,
}

pub fn offset(v: &Value) -> Option<Offset> {
    v.as_map()?;
    Some(Offset { x: v.get("x").as_double().unwrap_or(0.0), y: v.get("y").as_double().unwrap_or(0.0) })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    pub color: Color,
    pub offset: Offset,
    pub blur_radius: f64,
    pub spread_radius: f64,
}

pub fn box_shadow(v: &Value) -> BoxShadow {
    BoxShadow {
        color: color(v.get("color")).unwrap_or(Color::BLACK),
        offset: offset(v.get("offset")).unwrap_or(Offset { x: 0.0, y: 0.0 }),
        blur_radius: v.get("blurRadius").as_double().unwrap_or(0.0),
        spread_radius: v.get("spreadRadius").as_double().unwrap_or(0.0),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoxDecoration {
    pub color: Option<Color>,
    pub border: Option<Border>,
    pub border_radius: Option<BorderRadius>,
    pub box_shadow: Vec<BoxShadow>,
    pub shape: BoxShape,
}

pub fn decoration(v: &Value) -> Option<BoxDecoration> {
    if v.get("type").as_str()? != "box" {
        return None;
    }
    Some(BoxDecoration {
        color: color(v.get("color")),
        border: border(v.get("border")),
        border_radius: border_radius(v.get("borderRadius")),
        box_shadow: v.get("boxShadow").as_list().map(|l| l.iter().map(box_shadow).collect()).unwrap_or_default(),
        shape: BoxShape::decode(v.get("shape")).unwrap_or(BoxShape::Rectangle),
    })
}
