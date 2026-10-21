use comrak::nodes::{AlertType, AstNode, ListType, NodeValue, TableAlignment};
use comrak::{Anchorizer, Arena, Options, parse_document};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdAlert {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdAlign {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdKind {
    Document,
    Paragraph,
    Heading {
        level: u8,
        id: String,
    },
    BlockQuote,
    Alert {
        kind: MdAlert,
        title: Option<String>,
    },
    List {
        ordered: bool,
        start: usize,
        tight: bool,
    },
    Item {
        task: Option<bool>,
    },
    CodeBlock {
        info: String,
        literal: String,
    },
    HtmlBlock(String),
    ThematicBreak,
    Table {
        alignments: Vec<MdAlign>,
    },
    TableRow {
        header: bool,
    },
    TableCell,
    FootnoteDefinition {
        name: String,
    },
    FootnoteReference {
        name: String,
        index: u32,
    },
    Text(String),
    Code(String),
    Emph,
    Strong,
    Strikethrough,
    Link {
        url: String,
        title: String,
    },
    Image {
        url: String,
        title: String,
    },
    SoftBreak,
    LineBreak,
    HtmlInline(String),
    Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdNode {
    pub kind: MdKind,
    pub children: Vec<MdNode>,
}

impl MdNode {
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        match &self.kind {
            MdKind::Text(t) | MdKind::Code(t) => out.push_str(t),
            MdKind::CodeBlock { literal, .. } => out.push_str(literal),
            MdKind::SoftBreak | MdKind::LineBreak => out.push('\n'),
            _ => {}
        }
        for child in &self.children {
            child.collect_text(out);
        }
        if matches!(self.kind, MdKind::Paragraph | MdKind::Heading { .. }) {
            out.push('\n');
        }
    }
}

pub fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    let ext = &mut options.extension;
    ext.table = true;
    ext.tasklist = true;
    ext.strikethrough = true;
    ext.autolink = true;
    ext.footnotes = true;
    ext.alerts = true;
    ext.shortcodes = true;
    ext.header_ids = Some(String::new());
    options
}

pub fn parse_markdown(text: &str) -> MdNode {
    let arena = Arena::new();
    let root = parse_document(&arena, text, &markdown_options());
    let mut anchors = Anchorizer::new();
    convert(root, &mut anchors)
}

fn alert(kind: AlertType) -> MdAlert {
    match kind {
        AlertType::Note => MdAlert::Note,
        AlertType::Tip => MdAlert::Tip,
        AlertType::Important => MdAlert::Important,
        AlertType::Warning => MdAlert::Warning,
        AlertType::Caution => MdAlert::Caution,
    }
}

fn align(a: &TableAlignment) -> MdAlign {
    match a {
        TableAlignment::None => MdAlign::None,
        TableAlignment::Left => MdAlign::Left,
        TableAlignment::Center => MdAlign::Center,
        TableAlignment::Right => MdAlign::Right,
    }
}

fn convert<'a>(node: &'a AstNode<'a>, anchors: &mut Anchorizer) -> MdNode {
    let children: Vec<MdNode> = node.children().map(|c| convert(c, anchors)).collect();
    let kind = match &node.data.borrow().value {
        NodeValue::Document => MdKind::Document,
        NodeValue::Paragraph => MdKind::Paragraph,
        NodeValue::Heading(h) => {
            let text = MdNode {
                kind: MdKind::Span,
                children: children.clone(),
            }
            .plain_text();
            MdKind::Heading {
                level: h.level,
                id: anchors.anchorize(text),
            }
        }
        NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => MdKind::BlockQuote,
        NodeValue::Alert(a) => MdKind::Alert {
            kind: alert(a.alert_type),
            title: a.title.clone(),
        },
        NodeValue::List(l) => MdKind::List {
            ordered: l.list_type == ListType::Ordered,
            start: l.start,
            tight: l.tight,
        },
        NodeValue::Item(_) => MdKind::Item { task: None },
        NodeValue::TaskItem(mark) => MdKind::Item {
            task: Some(mark.is_some()),
        },
        NodeValue::CodeBlock(c) => MdKind::CodeBlock {
            info: c.info.clone(),
            literal: c.literal.clone(),
        },
        NodeValue::HtmlBlock(h) => MdKind::HtmlBlock(h.literal.clone()),
        NodeValue::ThematicBreak => MdKind::ThematicBreak,
        NodeValue::Table(t) => MdKind::Table {
            alignments: t.alignments.iter().map(align).collect(),
        },
        NodeValue::TableRow(header) => MdKind::TableRow { header: *header },
        NodeValue::TableCell => MdKind::TableCell,
        NodeValue::FootnoteDefinition(f) => MdKind::FootnoteDefinition {
            name: f.name.clone(),
        },
        NodeValue::FootnoteReference(f) => MdKind::FootnoteReference {
            name: f.name.clone(),
            index: f.ix,
        },
        NodeValue::Text(t) | NodeValue::Raw(t) | NodeValue::EscapedTag(t) => {
            MdKind::Text(t.clone())
        }
        NodeValue::ShortCode(s) => MdKind::Text(s.emoji.clone()),
        NodeValue::Code(c) => MdKind::Code(c.literal.clone()),
        NodeValue::Math(m) => MdKind::Code(m.literal.clone()),
        NodeValue::Emph => MdKind::Emph,
        NodeValue::Strong => MdKind::Strong,
        NodeValue::Strikethrough => MdKind::Strikethrough,
        NodeValue::Link(l) => MdKind::Link {
            url: l.url.clone(),
            title: l.title.clone(),
        },
        NodeValue::Image(l) => MdKind::Image {
            url: l.url.clone(),
            title: l.title.clone(),
        },
        NodeValue::SoftBreak => MdKind::SoftBreak,
        NodeValue::LineBreak => MdKind::LineBreak,
        NodeValue::HtmlInline(h) => MdKind::HtmlInline(h.clone()),
        _ => MdKind::Span,
    };
    MdNode { kind, children }
}
