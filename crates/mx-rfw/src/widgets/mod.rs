pub mod core_layout;
pub mod core_widgets;
pub mod maestria;

use crate::decoders::{Color, color};
use crate::tree::{LocalLibrary, Node};

use self::core_layout::{SingleChildScrollView, SizedBox, Stack, Text, Wrap};
use self::core_widgets::{
    Align, Center, Container, Expanded, Flex, FractionallySizedBox, GestureDetector, Opacity, Padding, Positioned,
    Rotation, ScrollList, SingleChild,
};
use self::maestria::{Draggable, DropTarget, Glyph, IconButton, Pressable, Svg, Tooltip};

pub struct CircularProgressIndicator {
    pub value: Option<f64>,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub stroke_width: f64,
    pub semantics_label: Option<String>,
    pub semantics_value: Option<String>,
}

impl CircularProgressIndicator {
    pub fn from_node(n: &Node) -> Self {
        Self {
            value: n.arg("value").as_double(),
            color: color(n.arg("color")),
            background_color: color(n.arg("backgroundColor")),
            stroke_width: n.arg("strokeWidth").as_double().unwrap_or(4.0),
            semantics_label: n.arg("semanticsLabel").as_str().map(str::to_owned),
            semantics_value: n.arg("semanticsValue").as_str().map(str::to_owned),
        }
    }
}

pub enum Local<'a> {
    Align(Align<'a>),
    Center(Center<'a>),
    Column(Flex<'a>),
    Row(Flex<'a>),
    Container(Box<Container<'a>>),
    Expanded(Expanded<'a>),
    Flexible(Expanded<'a>),
    FractionallySizedBox(FractionallySizedBox<'a>),
    GestureDetector(GestureDetector<'a>),
    GridView(ScrollList<'a>),
    IntrinsicHeight(SingleChild<'a>),
    ListView(ScrollList<'a>),
    Opacity(Opacity<'a>),
    Padding(Padding<'a>),
    Positioned(Positioned<'a>),
    Rotation(Rotation<'a>),
    SingleChildScrollView(SingleChildScrollView<'a>),
    SizedBox(SizedBox<'a>),
    Spacer { flex: i64 },
    Stack(Stack<'a>),
    Text(Box<Text>),
    Wrap(Wrap<'a>),
    CircularProgressIndicator(CircularProgressIndicator),
    Svg(Svg),
    Glyph(Glyph),
    IconButton(IconButton<'a>),
    Tooltip(Tooltip<'a>),
    Pressable(Box<Pressable<'a>>),
    MenuButton(Box<Pressable<'a>>),
    Draggable(Draggable<'a>),
    DropTarget(DropTarget<'a>),
    Unsupported(&'a Node),
}

pub fn decode(n: &Node) -> Local<'_> {
    match (n.library, n.name.as_str()) {
        (LocalLibrary::CoreWidgets, name) => match name {
            "Align" => Local::Align(Align::from_node(n)),
            "Center" => Local::Center(Center::from_node(n)),
            "Column" => Local::Column(Flex::from_node(n)),
            "Row" => Local::Row(Flex::from_node(n)),
            "Container" => Local::Container(Box::new(Container::from_node(n))),
            "Expanded" => Local::Expanded(Expanded::from_node(n)),
            "Flexible" => Local::Flexible(Expanded::flexible(n)),
            "FractionallySizedBox" => Local::FractionallySizedBox(FractionallySizedBox::from_node(n)),
            "GestureDetector" => Local::GestureDetector(GestureDetector::from_node(n)),
            "GridView" => Local::GridView(ScrollList::grid_view(n)),
            "IntrinsicHeight" => Local::IntrinsicHeight(SingleChild::from_node(n)),
            "ListView" => Local::ListView(ScrollList::list_view(n)),
            "Opacity" => Local::Opacity(Opacity::from_node(n)),
            "Padding" => Local::Padding(Padding::from_node(n)),
            "Positioned" => Local::Positioned(Positioned::from_node(n)),
            "Rotation" => Local::Rotation(Rotation::from_node(n)),
            "SingleChildScrollView" => Local::SingleChildScrollView(SingleChildScrollView::from_node(n)),
            "SizedBox" => Local::SizedBox(SizedBox::from_node(n)),
            "Spacer" => Local::Spacer { flex: n.arg("flex").as_int().unwrap_or(1) },
            "Stack" => Local::Stack(Stack::from_node(n)),
            "Text" => Local::Text(Box::new(Text::from_node(n))),
            "Wrap" => Local::Wrap(Wrap::from_node(n)),
            _ => Local::Unsupported(n),
        },
        (LocalLibrary::CoreMaterial, "CircularProgressIndicator") => {
            Local::CircularProgressIndicator(CircularProgressIndicator::from_node(n))
        }
        (LocalLibrary::Maestria, name) => match name {
            "Svg" => Local::Svg(Svg::from_node(n)),
            "Glyph" => Local::Glyph(Glyph::from_node(n)),
            "IconButton" => Local::IconButton(IconButton::from_node(n)),
            "Tooltip" => Local::Tooltip(Tooltip::from_node(n)),
            "Pressable" => Local::Pressable(Box::new(Pressable::from_node(n, false))),
            "MenuButton" => Local::MenuButton(Box::new(Pressable::from_node(n, true))),
            "Draggable" => Local::Draggable(Draggable::from_node(n)),
            "DropTarget" => Local::DropTarget(DropTarget::from_node(n)),
            _ => Local::Unsupported(n),
        },
        _ => Local::Unsupported(n),
    }
}
