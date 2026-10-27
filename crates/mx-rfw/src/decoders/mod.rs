mod decoration;
mod text;

pub use decoration::{
    Border, BorderRadius, BorderSide, BoxDecoration, BoxShadow, Offset, Radius, border, border_radius, border_side,
    box_shadow, decoration, offset, radius,
};
pub use text::{FontWeight, TextDecoration, TextStyle, font_weight, text_decoration, text_style};

use crate::value::Value;

macro_rules! rfw_enum {
    ($name:ident { $($variant:ident = $text:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn from_name(s: &str) -> Option<Self> {
                match s {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }

            pub fn decode(v: &Value) -> Option<Self> {
                v.as_str().and_then(Self::from_name)
            }
        }
    };
}

rfw_enum!(MainAxisAlignment { Start = "start", End = "end", Center = "center", SpaceBetween = "spaceBetween", SpaceAround = "spaceAround", SpaceEvenly = "spaceEvenly" });
rfw_enum!(MainAxisSize { Min = "min", Max = "max" });
rfw_enum!(CrossAxisAlignment { Start = "start", End = "end", Center = "center", Stretch = "stretch", Baseline = "baseline" });
rfw_enum!(TextDirection { Rtl = "rtl", Ltr = "ltr" });
rfw_enum!(VerticalDirection { Up = "up", Down = "down" });
rfw_enum!(TextBaseline { Alphabetic = "alphabetic", Ideographic = "ideographic" });
rfw_enum!(Clip { None = "none", HardEdge = "hardEdge", AntiAlias = "antiAlias", AntiAliasWithSaveLayer = "antiAliasWithSaveLayer" });
rfw_enum!(FlexFit { Tight = "tight", Loose = "loose" });
rfw_enum!(HitTestBehavior { DeferToChild = "deferToChild", Opaque = "opaque", Translucent = "translucent" });
rfw_enum!(Axis { Horizontal = "horizontal", Vertical = "vertical" });
rfw_enum!(StackFit { Loose = "loose", Expand = "expand", Passthrough = "passthrough" });
rfw_enum!(TextAlign { Left = "left", Right = "right", Center = "center", Justify = "justify", Start = "start", End = "end" });
rfw_enum!(TextOverflow { Clip = "clip", Fade = "fade", Ellipsis = "ellipsis", Visible = "visible" });
rfw_enum!(WrapAlignment { Start = "start", End = "end", Center = "center", SpaceBetween = "spaceBetween", SpaceAround = "spaceAround", SpaceEvenly = "spaceEvenly" });
rfw_enum!(WrapCrossAlignment { Start = "start", End = "end", Center = "center" });
rfw_enum!(BorderStyle { None = "none", Solid = "solid" });
rfw_enum!(BoxShape { Rectangle = "rectangle", Circle = "circle" });
rfw_enum!(FontStyle { Normal = "normal", Italic = "italic" });
rfw_enum!(TextDecorationStyle { Solid = "solid", Double = "double", Dotted = "dotted", Dashed = "dashed", Wavy = "wavy" });
rfw_enum!(TextLeadingDistribution { Proportional = "proportional", Even = "even" });
rfw_enum!(Curve {
    Linear = "linear", Decelerate = "decelerate", FastLinearToSlowEaseIn = "fastLinearToSlowEaseIn", Ease = "ease",
    EaseIn = "easeIn", EaseInToLinear = "easeInToLinear", EaseInSine = "easeInSine", EaseInQuad = "easeInQuad",
    EaseInCubic = "easeInCubic", EaseInQuart = "easeInQuart", EaseInQuint = "easeInQuint", EaseInExpo = "easeInExpo",
    EaseInCirc = "easeInCirc", EaseInBack = "easeInBack", EaseOut = "easeOut", LinearToEaseOut = "linearToEaseOut",
    EaseOutSine = "easeOutSine", EaseOutQuad = "easeOutQuad", EaseOutCubic = "easeOutCubic", EaseOutQuart = "easeOutQuart",
    EaseOutQuint = "easeOutQuint", EaseOutExpo = "easeOutExpo", EaseOutCirc = "easeOutCirc", EaseOutBack = "easeOutBack",
    EaseInOut = "easeInOut", EaseInOutSine = "easeInOutSine", EaseInOutQuad = "easeInOutQuad",
    EaseInOutCubic = "easeInOutCubic", EaseInOutCubicEmphasized = "easeInOutCubicEmphasized",
    EaseInOutQuart = "easeInOutQuart", EaseInOutQuint = "easeInOutQuint", EaseInOutExpo = "easeInOutExpo",
    EaseInOutCirc = "easeInOutCirc", EaseInOutBack = "easeInOutBack", FastOutSlowIn = "fastOutSlowIn",
    SlowMiddle = "slowMiddle", BounceIn = "bounceIn", BounceOut = "bounceOut", BounceInOut = "bounceInOut",
    ElasticIn = "elasticIn", ElasticOut = "elasticOut", ElasticInOut = "elasticInOut",
});

pub const DEFAULT_DURATION_MS: i64 = 200;

pub fn curve(v: &Value) -> Curve {
    Curve::decode(v).unwrap_or(Curve::FastOutSlowIn)
}

pub fn duration_ms(v: &Value) -> i64 {
    v.as_int().unwrap_or(DEFAULT_DURATION_MS)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color(pub u32);

impl Color {
    pub const BLACK: Color = Color(0xFF00_0000);

    pub fn alpha(self) -> u8 {
        (self.0 >> 24) as u8
    }

    pub fn red(self) -> u8 {
        (self.0 >> 16) as u8
    }

    pub fn green(self) -> u8 {
        (self.0 >> 8) as u8
    }

    pub fn blue(self) -> u8 {
        self.0 as u8
    }
}

pub fn color(v: &Value) -> Option<Color> {
    v.as_int().map(|i| Color(i as u32))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeInsets {
    pub start: f64,
    pub top: f64,
    pub end: f64,
    pub bottom: f64,
}

impl EdgeInsets {
    pub const ZERO: EdgeInsets = EdgeInsets { start: 0.0, top: 0.0, end: 0.0, bottom: 0.0 };
}

pub fn edge_insets(v: &Value) -> Option<EdgeInsets> {
    let a = v.at(0).as_double()?;
    let b = v.at(1).as_double();
    let c = v.at(2).as_double();
    let d = v.at(3).as_double();
    Some(EdgeInsets { start: a, top: b.unwrap_or(a), end: c.unwrap_or(a), bottom: d.or(b).unwrap_or(a) })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Alignment {
    Absolute { x: f64, y: f64 },
    Directional { start: f64, y: f64 },
}

impl Alignment {
    pub const CENTER: Alignment = Alignment::Absolute { x: 0.0, y: 0.0 };
    pub const TOP_START: Alignment = Alignment::Directional { start: -1.0, y: -1.0 };
}

pub fn alignment(v: &Value) -> Option<Alignment> {
    v.as_map()?;
    let x = v.get("x").as_double();
    let start = v.get("start").as_double();
    let y = v.get("y").as_double()?;
    match (start, x) {
        (Some(start), _) => Some(Alignment::Directional { start, y }),
        (None, Some(x)) => Some(Alignment::Absolute { x, y }),
        (None, None) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxConstraints {
    pub min_width: f64,
    pub max_width: f64,
    pub min_height: f64,
    pub max_height: f64,
}

pub fn box_constraints(v: &Value) -> Option<BoxConstraints> {
    v.as_map()?;
    Some(BoxConstraints {
        min_width: v.get("minWidth").as_double().unwrap_or(0.0),
        max_width: v.get("maxWidth").as_double().unwrap_or(f64::INFINITY),
        min_height: v.get("minHeight").as_double().unwrap_or(0.0),
        max_height: v.get("maxHeight").as_double().unwrap_or(f64::INFINITY),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridDelegate {
    FixedCrossAxisCount { cross_axis_count: i64, main_axis_spacing: f64, cross_axis_spacing: f64, child_aspect_ratio: f64, main_axis_extent: Option<f64> },
    MaxCrossAxisExtent { max_cross_axis_extent: f64, main_axis_spacing: f64, cross_axis_spacing: f64, child_aspect_ratio: f64, main_axis_extent: Option<f64> },
}

impl GridDelegate {
    pub const DEFAULT: GridDelegate = GridDelegate::FixedCrossAxisCount {
        cross_axis_count: 2,
        main_axis_spacing: 0.0,
        cross_axis_spacing: 0.0,
        child_aspect_ratio: 1.0,
        main_axis_extent: None,
    };
}

pub fn grid_delegate(v: &Value) -> Option<GridDelegate> {
    let d = |k: &str, default: f64| v.get(k).as_double().unwrap_or(default);
    let main_axis_extent = v.get("mainAxisExtent").as_double();
    match v.get("type").as_str()? {
        "fixedCrossAxisCount" => Some(GridDelegate::FixedCrossAxisCount {
            cross_axis_count: v.get("crossAxisCount").as_int().unwrap_or(2),
            main_axis_spacing: d("mainAxisSpacing", 0.0),
            cross_axis_spacing: d("crossAxisSpacing", 0.0),
            child_aspect_ratio: d("childAspectRatio", 1.0),
            main_axis_extent,
        }),
        "maxCrossAxisExtent" => Some(GridDelegate::MaxCrossAxisExtent {
            max_cross_axis_extent: d("maxCrossAxisExtent", 100.0),
            main_axis_spacing: d("mainAxisSpacing", 0.0),
            cross_axis_spacing: d("crossAxisSpacing", 0.0),
            child_aspect_ratio: d("childAspectRatio", 1.0),
            main_axis_extent,
        }),
        _ => None,
    }
}
