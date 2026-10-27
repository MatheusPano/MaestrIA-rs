use super::text_input::{number, rounded};
use crate::decoders::{Alignment, Clip, alignment};
use crate::tree::{Event, Node, Widget};
use crate::value::{Map, Value};

pub const STEP: f64 = 1.5;
pub const SCALE_FACTOR: f64 = 200.0;
pub const ANIMATION_MS: u64 = 220;
pub const SETTLE_MS: u64 = 200;
pub const REPORT_EPSILON: f64 = 0.005;

pub struct ZoomView<'a> {
    pub child: Option<&'a Widget>,
    pub max_scale: f64,
    pub command: Option<String>,
    pub command_rev: i64,
    pub on_zoom: Vec<&'a Event>,
}

impl<'a> ZoomView<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            child: n.optional_child("child"),
            max_scale: number(n.arg("maxScale")).unwrap_or(6.0).max(1.0),
            command: n.arg("command").as_str().map(str::to_owned),
            command_rev: rounded(n.arg("commandRev")).unwrap_or(0),
            on_zoom: n.handlers("onZoom"),
        }
    }
}

pub fn zoom_extra(scale: f64) -> Map {
    Map::from_iter([("scale".to_owned(), Value::Double(scale))])
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Transform = Transform { scale: 1.0, x: 0.0, y: 0.0 };

    pub fn to_scene(&self, p: (f64, f64)) -> (f64, f64) {
        ((p.0 - self.x) / self.scale, (p.1 - self.y) / self.scale)
    }

    pub fn lerp(&self, to: &Transform, t: f64) -> Transform {
        Transform {
            scale: self.scale + (to.scale - self.scale) * t,
            x: self.x + (to.x - self.x) * t,
            y: self.y + (to.y - self.y) * t,
        }
    }
}

pub fn clamp_translation(m: Transform, size: (f64, f64)) -> Transform {
    let min_x = (size.0 - size.0 * m.scale).min(0.0);
    let min_y = (size.1 - size.1 * m.scale).min(0.0);
    Transform { scale: m.scale, x: m.x.clamp(min_x, 0.0), y: m.y.clamp(min_y, 0.0) }
}

pub fn zoom_target(command: Option<&str>, scale: f64, max_scale: f64) -> Option<f64> {
    let target = match command? {
        "in" => scale * STEP,
        "out" => scale / STEP,
        "fit" => 1.0,
        _ => return None,
    };
    Some(target.clamp(1.0, max_scale))
}

pub fn command_transform(current: Transform, size: (f64, f64), to: f64) -> Transform {
    if to <= 1.0001 {
        return Transform::IDENTITY;
    }
    let center = (size.0 / 2.0, size.1 / 2.0);
    let focal = current.to_scene(center);
    clamp_translation(Transform { scale: to, x: center.0 - focal.0 * to, y: center.1 - focal.1 * to }, size)
}

pub fn zoom_around(current: Transform, focal: (f64, f64), factor: f64, max_scale: f64, size: (f64, f64)) -> Transform {
    let scale = (current.scale * factor).clamp(1.0, max_scale);
    let k = scale / current.scale;
    let m = Transform { scale, x: focal.0 - (focal.0 - current.x) * k, y: focal.1 - (focal.1 - current.y) * k };
    clamp_translation(m, size)
}

pub fn wheel_factor(delta_y: f64) -> f64 {
    (-delta_y / SCALE_FACTOR).exp()
}

pub fn pan(current: Transform, dx: f64, dy: f64, size: (f64, f64)) -> Transform {
    clamp_translation(Transform { x: current.x + dx, y: current.y + dy, ..current }, size)
}

pub fn should_report(scale: f64, reported: f64) -> bool {
    (scale - reported).abs() >= REPORT_EPSILON
}

pub fn ease_out_cubic(t: f64) -> f64 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

pub fn ease_in_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxFit {
    Fill,
    Contain,
    Cover,
    FitWidth,
    FitHeight,
    None,
    ScaleDown,
}

impl BoxFit {
    pub fn decode(v: &Value) -> Option<Self> {
        Some(match v.as_str()? {
            "fill" => BoxFit::Fill,
            "contain" => BoxFit::Contain,
            "cover" => BoxFit::Cover,
            "fitWidth" => BoxFit::FitWidth,
            "fitHeight" => BoxFit::FitHeight,
            "none" => BoxFit::None,
            "scaleDown" => BoxFit::ScaleDown,
            _ => return None,
        })
    }
}

pub struct FittedBox<'a> {
    pub fit: BoxFit,
    pub alignment: Alignment,
    pub clip: bool,
    pub child: Option<&'a Widget>,
}

impl<'a> FittedBox<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            fit: BoxFit::decode(n.arg("fit")).unwrap_or(BoxFit::Contain),
            alignment: alignment(n.arg("alignment")).unwrap_or(Alignment::CENTER),
            clip: Clip::decode(n.arg("clipBehavior")).is_some_and(|c| c != Clip::None),
            child: n.optional_child("child"),
        }
    }
}

pub fn fitted_scale(fit: BoxFit, child: (f64, f64), outer: (f64, f64)) -> f64 {
    if child.0 <= 0.0 || child.1 <= 0.0 {
        return 1.0;
    }
    let (sx, sy) = (outer.0 / child.0, outer.1 / child.1);
    let scale = match fit {
        BoxFit::Contain | BoxFit::Fill => sx.min(sy),
        BoxFit::Cover => sx.max(sy),
        BoxFit::FitWidth => sx,
        BoxFit::FitHeight => sy,
        BoxFit::None => 1.0,
        BoxFit::ScaleDown => sx.min(sy).min(1.0),
    };
    if scale.is_finite() && scale > 0.0 { scale } else { 1.0 }
}
