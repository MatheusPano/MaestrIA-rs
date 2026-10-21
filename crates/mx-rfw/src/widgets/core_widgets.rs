use crate::decoders::{
    Alignment, Axis, BoxConstraints, BoxDecoration, Clip, Color, CrossAxisAlignment, Curve, EdgeInsets, FlexFit,
    GridDelegate, HitTestBehavior, MainAxisAlignment, MainAxisSize, TextBaseline, TextDirection, VerticalDirection,
    alignment, box_constraints, color, curve, decoration, duration_ms, edge_insets, grid_delegate,
};
use crate::tree::{Child, Event, Node, Widget};

fn d(n: &Node, k: &str) -> Option<f64> {
    n.arg(k).as_double()
}

fn b(n: &Node, k: &str) -> Option<bool> {
    n.arg(k).as_bool()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animation {
    pub duration_ms: i64,
    pub curve: Curve,
}

impl Animation {
    fn of(n: &Node) -> Self {
        Self { duration_ms: duration_ms(n.arg("duration")), curve: curve(n.arg("curve")) }
    }
}

pub struct Align<'a> {
    pub animation: Animation,
    pub alignment: Alignment,
    pub width_factor: Option<f64>,
    pub height_factor: Option<f64>,
    pub on_end: Vec<&'a Event>,
    pub child: Option<&'a Widget>,
}

impl<'a> Align<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            animation: Animation::of(n),
            alignment: alignment(n.arg("alignment")).unwrap_or(Alignment::CENTER),
            width_factor: d(n, "widthFactor"),
            height_factor: d(n, "heightFactor"),
            on_end: n.handlers("onEnd"),
            child: n.optional_child("child"),
        }
    }
}

pub struct Center<'a> {
    pub width_factor: Option<f64>,
    pub height_factor: Option<f64>,
    pub child: Option<&'a Widget>,
}

impl<'a> Center<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self { width_factor: d(n, "widthFactor"), height_factor: d(n, "heightFactor"), child: n.optional_child("child") }
    }
}

pub struct Flex<'a> {
    pub main_axis_alignment: MainAxisAlignment,
    pub main_axis_size: MainAxisSize,
    pub cross_axis_alignment: CrossAxisAlignment,
    pub text_direction: Option<TextDirection>,
    pub vertical_direction: VerticalDirection,
    pub text_baseline: Option<TextBaseline>,
    pub children: Vec<Child<'a>>,
}

impl<'a> Flex<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            main_axis_alignment: MainAxisAlignment::decode(n.arg("mainAxisAlignment")).unwrap_or(MainAxisAlignment::Start),
            main_axis_size: MainAxisSize::decode(n.arg("mainAxisSize")).unwrap_or(MainAxisSize::Max),
            cross_axis_alignment: CrossAxisAlignment::decode(n.arg("crossAxisAlignment"))
                .unwrap_or(CrossAxisAlignment::Center),
            text_direction: TextDirection::decode(n.arg("textDirection")),
            vertical_direction: VerticalDirection::decode(n.arg("verticalDirection")).unwrap_or(VerticalDirection::Down),
            text_baseline: TextBaseline::decode(n.arg("textBaseline")),
            children: n.child_list("children"),
        }
    }
}

pub struct Container<'a> {
    pub animation: Animation,
    pub alignment: Option<Alignment>,
    pub padding: Option<EdgeInsets>,
    pub color: Option<Color>,
    pub decoration: Option<BoxDecoration>,
    pub foreground_decoration: Option<BoxDecoration>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub constraints: Option<BoxConstraints>,
    pub margin: Option<EdgeInsets>,
    pub clip_behavior: Clip,
    pub on_end: Vec<&'a Event>,
    pub child: Option<&'a Widget>,
}

impl<'a> Container<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            animation: Animation::of(n),
            alignment: alignment(n.arg("alignment")),
            padding: edge_insets(n.arg("padding")),
            color: color(n.arg("color")),
            decoration: decoration(n.arg("decoration")),
            foreground_decoration: decoration(n.arg("foregroundDecoration")),
            width: d(n, "width"),
            height: d(n, "height"),
            constraints: box_constraints(n.arg("constraints")),
            margin: edge_insets(n.arg("margin")),
            clip_behavior: Clip::decode(n.arg("clipBehavior")).unwrap_or(Clip::None),
            on_end: n.handlers("onEnd"),
            child: n.optional_child("child"),
        }
    }
}

pub struct Expanded<'a> {
    pub flex: i64,
    pub fit: FlexFit,
    pub child: Child<'a>,
}

impl<'a> Expanded<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self { flex: n.arg("flex").as_int().unwrap_or(1), fit: FlexFit::Tight, child: n.child("child") }
    }

    pub fn flexible(n: &'a Node) -> Self {
        Self {
            flex: n.arg("flex").as_int().unwrap_or(1),
            fit: FlexFit::decode(n.arg("fit")).unwrap_or(FlexFit::Loose),
            child: n.child("child"),
        }
    }
}

pub struct FractionallySizedBox<'a> {
    pub alignment: Alignment,
    pub width_factor: Option<f64>,
    pub height_factor: Option<f64>,
    pub child: Child<'a>,
}

impl<'a> FractionallySizedBox<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            alignment: alignment(n.arg("alignment")).unwrap_or(Alignment::CENTER),
            width_factor: d(n, "widthFactor"),
            height_factor: d(n, "heightFactor"),
            child: n.child("child"),
        }
    }
}

pub struct GestureDetector<'a> {
    pub on_tap: Vec<&'a Event>,
    pub on_tap_down: Vec<&'a Event>,
    pub on_tap_up: Vec<&'a Event>,
    pub on_tap_cancel: Vec<&'a Event>,
    pub on_double_tap: Vec<&'a Event>,
    pub on_long_press: Vec<&'a Event>,
    pub behavior: Option<HitTestBehavior>,
    pub child: Option<&'a Widget>,
}

impl<'a> GestureDetector<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            on_tap: n.handlers("onTap"),
            on_tap_down: n.handlers("onTapDown"),
            on_tap_up: n.handlers("onTapUp"),
            on_tap_cancel: n.handlers("onTapCancel"),
            on_double_tap: n.handlers("onDoubleTap"),
            on_long_press: n.handlers("onLongPress"),
            behavior: HitTestBehavior::decode(n.arg("behavior")),
            child: n.optional_child("child"),
        }
    }
}

pub struct ScrollList<'a> {
    pub scroll_direction: Axis,
    pub reverse: bool,
    pub primary: Option<bool>,
    pub shrink_wrap: bool,
    pub padding: Option<EdgeInsets>,
    pub item_extent: Option<f64>,
    pub prototype_item: Option<&'a Widget>,
    pub grid_delegate: Option<GridDelegate>,
    pub clip_behavior: Clip,
    pub children: Vec<Child<'a>>,
}

impl<'a> ScrollList<'a> {
    pub fn list_view(n: &'a Node) -> Self {
        Self { grid_delegate: None, ..Self::common(n) }
    }

    pub fn grid_view(n: &'a Node) -> Self {
        Self {
            item_extent: None,
            prototype_item: None,
            grid_delegate: Some(grid_delegate(n.arg("gridDelegate")).unwrap_or(GridDelegate::DEFAULT)),
            ..Self::common(n)
        }
    }

    fn common(n: &'a Node) -> Self {
        Self {
            scroll_direction: Axis::decode(n.arg("scrollDirection")).unwrap_or(Axis::Vertical),
            reverse: b(n, "reverse").unwrap_or(false),
            primary: b(n, "primary"),
            shrink_wrap: b(n, "shrinkWrap").unwrap_or(false),
            padding: edge_insets(n.arg("padding")),
            item_extent: d(n, "itemExtent"),
            prototype_item: n.optional_child("prototypeItem"),
            grid_delegate: None,
            clip_behavior: Clip::decode(n.arg("clipBehavior")).unwrap_or(Clip::HardEdge),
            children: n.indexed_children("children"),
        }
    }
}

pub struct SingleChild<'a> {
    pub animation: Animation,
    pub on_end: Vec<&'a Event>,
    pub child: Option<&'a Widget>,
}

impl<'a> SingleChild<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self { animation: Animation::of(n), on_end: n.handlers("onEnd"), child: n.optional_child("child") }
    }
}

pub struct Opacity<'a> {
    pub opacity: f64,
    pub always_include_semantics: bool,
    pub base: SingleChild<'a>,
}

impl<'a> Opacity<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            opacity: d(n, "opacity").unwrap_or(0.0),
            always_include_semantics: b(n, "alwaysIncludeSemantics").unwrap_or(true),
            base: SingleChild::from_node(n),
        }
    }
}

pub struct Padding<'a> {
    pub padding: EdgeInsets,
    pub base: SingleChild<'a>,
}

impl<'a> Padding<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self { padding: edge_insets(n.arg("padding")).unwrap_or(EdgeInsets::ZERO), base: SingleChild::from_node(n) }
    }
}

pub struct Positioned<'a> {
    pub animation: Animation,
    pub start: Option<f64>,
    pub top: Option<f64>,
    pub end: Option<f64>,
    pub bottom: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub on_end: Vec<&'a Event>,
    pub child: Child<'a>,
}

impl<'a> Positioned<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            animation: Animation::of(n),
            start: d(n, "start"),
            top: d(n, "top"),
            end: d(n, "end"),
            bottom: d(n, "bottom"),
            width: d(n, "width"),
            height: d(n, "height"),
            on_end: n.handlers("onEnd"),
            child: n.child("child"),
        }
    }
}

pub struct Rotation<'a> {
    pub turns: f64,
    pub alignment: Alignment,
    pub base: SingleChild<'a>,
}

impl<'a> Rotation<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            turns: d(n, "turns").unwrap_or(0.0),
            alignment: alignment(n.arg("alignment")).unwrap_or(Alignment::CENTER),
            base: SingleChild::from_node(n),
        }
    }
}
