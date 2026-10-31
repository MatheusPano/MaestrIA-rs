use super::text_input::number;
use super::zoom_view::BoxFit;
use crate::tree::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub path: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub fit: BoxFit,
}

impl Picture {
    pub fn from_node(n: &Node) -> Self {
        Self {
            path: n.arg("path").as_str().map(str::to_owned),
            width: number(n.arg("width")),
            height: number(n.arg("height")),
            fit: BoxFit::decode(n.arg("fit")).unwrap_or(BoxFit::Contain),
        }
    }
}
