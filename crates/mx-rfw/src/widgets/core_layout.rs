use crate::decoders::{
    Alignment, Axis, Clip, EdgeInsets, StackFit, TextAlign, TextDirection, TextOverflow, TextStyle, VerticalDirection,
    WrapAlignment, WrapCrossAlignment, alignment, edge_insets, text_style,
};
use crate::tree::{Child, Node, Widget};

fn d(n: &Node, k: &str) -> Option<f64> {
    n.arg(k).as_double()
}

fn b(n: &Node, k: &str) -> Option<bool> {
    n.arg(k).as_bool()
}

pub struct SingleChildScrollView<'a> {
    pub scroll_direction: Axis,
    pub reverse: bool,
    pub padding: Option<EdgeInsets>,
    pub primary: bool,
    pub clip_behavior: Clip,
    pub child: Option<&'a Widget>,
}

impl<'a> SingleChildScrollView<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            scroll_direction: Axis::decode(n.arg("scrollDirection")).unwrap_or(Axis::Vertical),
            reverse: b(n, "reverse").unwrap_or(false),
            padding: edge_insets(n.arg("padding")),
            primary: b(n, "primary").unwrap_or(true),
            clip_behavior: Clip::decode(n.arg("clipBehavior")).unwrap_or(Clip::HardEdge),
            child: n.optional_child("child"),
        }
    }
}

pub struct SizedBox<'a> {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub child: Option<&'a Widget>,
}

impl<'a> SizedBox<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self { width: d(n, "width"), height: d(n, "height"), child: n.optional_child("child") }
    }
}

pub struct Stack<'a> {
    pub alignment: Alignment,
    pub text_direction: Option<TextDirection>,
    pub fit: StackFit,
    pub clip_behavior: Clip,
    pub children: Vec<Child<'a>>,
}

impl<'a> Stack<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            alignment: alignment(n.arg("alignment")).unwrap_or(Alignment::TOP_START),
            text_direction: TextDirection::decode(n.arg("textDirection")),
            fit: StackFit::decode(n.arg("fit")).unwrap_or(StackFit::Loose),
            clip_behavior: Clip::decode(n.arg("clipBehavior")).unwrap_or(Clip::HardEdge),
            children: n.child_list("children"),
        }
    }
}

pub struct Text {
    pub text: String,
    pub style: Option<TextStyle>,
    pub text_align: Option<TextAlign>,
    pub text_direction: Option<TextDirection>,
    pub soft_wrap: Option<bool>,
    pub overflow: Option<TextOverflow>,
    pub text_scale_factor: Option<f64>,
    pub max_lines: Option<i64>,
    pub semantics_label: Option<String>,
}

impl Text {
    pub fn from_node(n: &Node) -> Self {
        let raw = n.arg("text");
        let text = match raw.as_str() {
            Some(s) => s.to_owned(),
            None => raw.as_list().unwrap_or_default().iter().filter_map(|v| v.as_str()).collect(),
        };
        Self {
            text,
            style: text_style(n, "style"),
            text_align: TextAlign::decode(n.arg("textAlign")),
            text_direction: TextDirection::decode(n.arg("textDirection")),
            soft_wrap: b(n, "softWrap"),
            overflow: TextOverflow::decode(n.arg("overflow")),
            text_scale_factor: d(n, "textScaleFactor"),
            max_lines: n.arg("maxLines").as_int(),
            semantics_label: n.arg("semanticsLabel").as_str().map(str::to_owned),
        }
    }
}

pub struct Wrap<'a> {
    pub direction: Axis,
    pub alignment: WrapAlignment,
    pub spacing: f64,
    pub run_alignment: WrapAlignment,
    pub run_spacing: f64,
    pub cross_axis_alignment: WrapCrossAlignment,
    pub text_direction: Option<TextDirection>,
    pub vertical_direction: VerticalDirection,
    pub clip_behavior: Clip,
    pub children: Vec<Child<'a>>,
}

impl<'a> Wrap<'a> {
    pub fn from_node(n: &'a Node) -> Self {
        Self {
            direction: Axis::decode(n.arg("direction")).unwrap_or(Axis::Horizontal),
            alignment: WrapAlignment::decode(n.arg("alignment")).unwrap_or(WrapAlignment::Start),
            spacing: d(n, "spacing").unwrap_or(0.0),
            run_alignment: WrapAlignment::decode(n.arg("runAlignment")).unwrap_or(WrapAlignment::Start),
            run_spacing: d(n, "runSpacing").unwrap_or(0.0),
            cross_axis_alignment: WrapCrossAlignment::decode(n.arg("crossAxisAlignment"))
                .unwrap_or(WrapCrossAlignment::Start),
            text_direction: TextDirection::decode(n.arg("textDirection")),
            vertical_direction: VerticalDirection::decode(n.arg("verticalDirection")).unwrap_or(VerticalDirection::Down),
            clip_behavior: Clip::decode(n.arg("clipBehavior")).unwrap_or(Clip::None),
            children: n.child_list("children"),
        }
    }
}
