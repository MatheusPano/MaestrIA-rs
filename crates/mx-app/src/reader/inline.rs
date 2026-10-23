use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    StyledText, TextRun, Window, div,
};
use mx_claude::markdown::{MdKind, MdNode};

use super::style::{Marks, Sheet, run};

pub type FollowHref = Rc<dyn Fn(&str, &mut Window, &mut App)>;

pub struct Inline {
    pub text: String,
    runs: Vec<TextRun>,
    links: Vec<(Range<usize>, String)>,
}

impl Inline {
    pub fn collect(nodes: &[MdNode], base: Hsla, marks: Marks, sheet: &Sheet) -> Self {
        let mut inline = Self {
            text: String::new(),
            runs: Vec::new(),
            links: Vec::new(),
        };
        for node in nodes {
            inline.walk(node, base, marks, sheet);
        }
        inline
    }

    pub fn plain(text: &str, base: Hsla, marks: Marks, sheet: &Sheet) -> Self {
        let mut inline = Self::collect(&[], base, marks, sheet);
        inline.push(text, base, marks, sheet);
        inline
    }

    pub fn runs(&self) -> &[TextRun] {
        &self.runs
    }

    pub fn run_at(&self, offset: usize) -> Option<&TextRun> {
        let mut end = 0;
        self.runs.iter().find(|r| {
            end += r.len;
            offset < end
        })
    }

    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    fn push(&mut self, text: &str, base: Hsla, marks: Marks, sheet: &Sheet) {
        if text.is_empty() {
            return;
        }
        self.text.push_str(text);
        self.runs.push(run(text.len(), marks, base, sheet));
    }

    fn walk(&mut self, node: &MdNode, base: Hsla, marks: Marks, sheet: &Sheet) {
        let mut inner = marks;
        match &node.kind {
            MdKind::Text(text) => return self.push(text, base, marks, sheet),
            MdKind::Code(text) => {
                inner.code = true;
                return self.push(&text.replace('\n', " "), base, inner, sheet);
            }
            MdKind::SoftBreak => return self.push(" ", base, marks, sheet),
            MdKind::LineBreak => return self.push("\n", base, marks, sheet),
            MdKind::HtmlInline(html) => {
                if is_break_tag(html) {
                    self.push("\n", base, marks, sheet);
                }
                return;
            }
            MdKind::FootnoteReference { index, .. } => {
                inner.link = true;
                return self.push(&format!("[{index}]"), base, inner, sheet);
            }
            MdKind::Emph => inner.italic = true,
            MdKind::Strong => inner.bold = true,
            MdKind::Strikethrough => inner.strike = true,
            MdKind::Image { url, title } => {
                inner.italic = true;
                inner.faint = true;
                let before = self.text.len();
                self.walk_children(node, base, inner, sheet);
                if self.text.len() == before {
                    let alt = if title.is_empty() { url } else { title };
                    self.push(alt, base, inner, sheet);
                }
                return;
            }
            MdKind::Link { url, .. } => {
                inner.link = true;
                let start = self.text.len();
                self.walk_children(node, base, inner, sheet);
                if self.text.len() == start {
                    self.push(url, base, inner, sheet);
                }
                self.links.push((start..self.text.len(), url.clone()));
                return;
            }
            _ => {}
        }
        self.walk_children(node, base, inner, sheet);
    }

    fn walk_children(&mut self, node: &MdNode, base: Hsla, marks: Marks, sheet: &Sheet) {
        for child in &node.children {
            self.walk(child, base, marks, sheet);
        }
    }

    pub fn into_element(self, id: ElementId, follow: Option<&FollowHref>) -> AnyElement {
        let text = StyledText::new(self.text).with_runs(self.runs);
        match follow {
            Some(follow) if !self.links.is_empty() => {
                let follow = follow.clone();
                let layout = text.layout().clone();
                let links = self.links;
                div()
                    .id(id)
                    .on_click(move |ev, window, cx| {
                        let Ok(at) = layout.index_for_position(ev.position()) else { return };
                        if let Some((_, url)) = links.iter().find(|(r, _)| r.contains(&at)) {
                            follow(url, window, cx);
                        }
                    })
                    .child(text)
                    .into_any_element()
            }
            _ => text.into_any_element(),
        }
    }
}

fn is_break_tag(html: &str) -> bool {
    let tag: String = html
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '/')
        .collect();
    tag.eq_ignore_ascii_case("<br>")
}
