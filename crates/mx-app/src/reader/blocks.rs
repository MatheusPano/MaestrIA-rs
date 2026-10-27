use std::cell::Cell;
use std::sync::Arc;

use gpui::{
    AnyElement, Div, ElementId, FontWeight, Hsla, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, StyledText, WindowTextSystem, div, prelude::*, px,
    relative,
};
use mx_claude::markdown::{MdAlert, MdKind, MdNode};

use super::inline::{FollowHref, Inline};
use super::select::{self, SelectionSlot};
use super::style::{Marks, Sheet, run};

pub const BLOCK_SPACING: f32 = 11.;
const LIST_INDENT: f32 = 22.;

pub struct Painter<'a> {
    pub sheet: &'a Sheet,
    pub follow: Option<FollowHref>,
    pub text_system: Arc<WindowTextSystem>,
    pub width: Pixels,
    pub selection: Option<SelectionSlot>,
    next_id: Cell<usize>,
}

impl<'a> Painter<'a> {
    pub fn new(
        sheet: &'a Sheet,
        follow: Option<FollowHref>,
        text_system: Arc<WindowTextSystem>,
        width: Pixels,
    ) -> Self {
        Self {
            sheet,
            follow,
            text_system,
            width,
            selection: None,
            next_id: Cell::new(0),
        }
    }

    pub fn with_selection(mut self, slot: Option<SelectionSlot>) -> Self {
        self.selection = slot;
        self
    }

    fn text(&self, text: String, runs: Option<Vec<gpui::TextRun>>) -> AnyElement {
        match &self.selection {
            Some(slot) => select::piece(slot, text, runs),
            None => match runs {
                Some(runs) => StyledText::new(text).with_runs(runs).into_any_element(),
                None => text.into_any_element(),
            },
        }
    }

    pub fn id(&self, name: &'static str) -> ElementId {
        let n = self.next_id.get();
        self.next_id.set(n + 1);
        ElementId::named_usize(name, n)
    }

    pub fn document(&self, root: &MdNode) -> Div {
        let (notes, body): (Vec<&MdNode>, Vec<&MdNode>) = root
            .children
            .iter()
            .partition(|n| matches!(n.kind, MdKind::FootnoteDefinition { .. }));
        let mut column = self.stack(body.into_iter(), self.sheet.fg, BLOCK_SPACING);
        if !notes.is_empty() {
            column = column.child(self.footnotes(&notes));
        }
        column
    }

    pub fn stack<'n>(&self, nodes: impl Iterator<Item = &'n MdNode>, base: Hsla, gap: f32) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(gap))
            .children(nodes.filter_map(|n| self.block(n, base)))
    }

    pub fn paragraph(&self, inline: Inline, size: f32) -> AnyElement {
        div()
            .text_size(px(size))
            .line_height(relative(1.62))
            .pb(px(2.))
            .child(inline.into_element(self.id("p"), self.follow.as_ref(), self.selection.as_ref()))
            .into_any_element()
    }

    fn block(&self, node: &MdNode, base: Hsla) -> Option<AnyElement> {
        let sheet = self.sheet;
        Some(match &node.kind {
            MdKind::Paragraph => {
                let inline = Inline::collect(&node.children, base, Marks::default(), sheet);
                if inline.is_blank() {
                    return None;
                }
                self.paragraph(inline, sheet.size)
            }
            MdKind::Heading { level, .. } => self.heading(node, *level),
            MdKind::BlockQuote => self.quote(node, sheet.quote_bar, None),
            MdKind::Alert { kind, title } => self.alert(node, *kind, title.as_deref()),
            MdKind::List { ordered, start, tight } => {
                self.list(node, *ordered, *start, *tight, base)
            }
            MdKind::CodeBlock { literal, .. } => self.code(literal),
            MdKind::HtmlBlock(html) => {
                let text = strip_tags(html);
                if text.trim().is_empty() {
                    return None;
                }
                self.paragraph(Inline::plain(text.trim(), base, Marks::default(), sheet), sheet.size)
            }
            MdKind::ThematicBreak => div()
                .my(px(4.))
                .border_t_1()
                .border_color(sheet.border)
                .into_any_element(),
            MdKind::Table { .. } => self.table(node),
            MdKind::FootnoteDefinition { .. } => return None,
            _ => {
                let inline = Inline::collect(std::slice::from_ref(node), base, Marks::default(), sheet);
                if inline.is_blank() {
                    return None;
                }
                self.paragraph(inline, sheet.size)
            }
        })
    }

    fn heading(&self, node: &MdNode, level: u8) -> AnyElement {
        let (size, color, top, bottom) = self.sheet.heading(level);
        let marks = Marks {
            bold: true,
            ..Marks::default()
        };
        let inline = Inline::collect(&node.children, color, marks, self.sheet);
        div()
            .pt(px(top))
            .pb(px(bottom))
            .text_size(px(size))
            .line_height(relative(1.3))
            .font_weight(FontWeight::SEMIBOLD)
            .child(inline.into_element(self.id("h"), self.follow.as_ref(), self.selection.as_ref()))
            .into_any_element()
    }

    fn quote(&self, node: &MdNode, bar: Hsla, title: Option<(Hsla, &str)>) -> AnyElement {
        let sheet = self.sheet;
        let mut body = self.stack(node.children.iter(), sheet.fg_dim, BLOCK_SPACING);
        if let Some((color, label)) = title {
            let marks = Marks {
                bold: true,
                ..Marks::default()
            };
            let head = self.paragraph(Inline::plain(label, color, marks, sheet), sheet.size);
            body = div().flex().flex_col().gap(px(4.)).child(head).child(body);
        }
        div()
            .bg(sheet.bg_hover)
            .rounded(px(6.))
            .border_l(px(3.))
            .border_color(bar)
            .pl(px(14.))
            .pt(px(10.))
            .pr(px(12.))
            .pb(px(10.))
            .child(body)
            .into_any_element()
    }

    fn alert(&self, node: &MdNode, kind: MdAlert, title: Option<&str>) -> AnyElement {
        let (color, label) = self.sheet.alert(kind);
        let label = title.filter(|t| !t.trim().is_empty()).unwrap_or(label);
        self.quote(node, color.opacity(0.8), Some((color, label)))
    }

    fn list(&self, node: &MdNode, ordered: bool, start: usize, tight: bool, base: Hsla) -> AnyElement {
        let sheet = self.sheet;
        let line = sheet.size * 1.62;
        let items = node.children.iter().enumerate().map(|(i, item)| {
            let task = match item.kind {
                MdKind::Item { task } => task,
                _ => None,
            };
            let marker = match task {
                Some(done) => checkbox(sheet, done),
                None => {
                    let text = if ordered {
                        format!("{}.", start + i)
                    } else {
                        "•".to_owned()
                    };
                    div()
                        .text_size(px(sheet.size))
                        .line_height(relative(1.62))
                        .text_color(base)
                        .child(self.text(text, None))
                        .into_any_element()
                }
            };
            let gap = if tight { 2. } else { BLOCK_SPACING };
            div()
                .flex()
                .flex_row()
                .child(
                    div()
                        .flex_none()
                        .w(px(LIST_INDENT))
                        .h(px(line))
                        .pr(px(6.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .child(marker),
                )
                .child(
                    self.stack(item.children.iter(), base, gap)
                        .flex_1()
                        .min_w_0(),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap(px(if tight { 2. } else { 6. }))
            .children(items)
            .into_any_element()
    }

    fn code(&self, literal: &str) -> AnyElement {
        let sheet = self.sheet;
        let text = literal.trim_end_matches('\n').replace('\t', "    ");
        let runs = vec![run(text.len(), Marks { code: true, ..Marks::default() }, sheet.fg, sheet)];
        div()
            .bg(sheet.bg_active)
            .border_1()
            .border_color(sheet.border)
            .rounded(px(8.))
            .child(
                div()
                    .id(self.id("code"))
                    .overflow_x_scroll()
                    .p(px(12.))
                    .child(
                        div()
                            .text_size(px(sheet.mono_size()))
                            .line_height(relative(1.5))
                            .whitespace_nowrap()
                            .child(self.text(text, Some(runs))),
                    ),
            )
            .into_any_element()
    }

    fn footnotes(&self, notes: &[&MdNode]) -> AnyElement {
        let sheet = self.sheet;
        let size = sheet.size - 1.;
        let rows = notes.iter().enumerate().map(|(i, note)| {
            div()
                .flex()
                .flex_row()
                .text_size(px(size))
                .line_height(relative(1.62))
                .text_color(sheet.fg_dim)
                .child(
                    div()
                        .flex_none()
                        .w(px(LIST_INDENT))
                        .pr(px(6.))
                        .flex()
                        .justify_end()
                        .child(format!("{}.", i + 1)),
                )
                .child(self.stack(note.children.iter(), sheet.fg_dim, 4.).flex_1().min_w_0())
        });
        div()
            .mt(px(8.))
            .pt(px(BLOCK_SPACING))
            .border_t_1()
            .border_color(sheet.border)
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(rows)
            .into_any_element()
    }
}

fn checkbox(sheet: &Sheet, done: bool) -> AnyElement {
    let size = (sheet.size - 0.5).round();
    let mut tick = div()
        .size(px(size))
        .rounded(px(3.))
        .border(px(1.5))
        .border_color(sheet.accent)
        .flex()
        .items_center()
        .justify_center();
    if done {
        tick = tick
            .bg(sheet.accent)
            .text_color(sheet.on_accent)
            .text_size(px(size - 3.))
            .line_height(px(size))
            .font_weight(FontWeight::BOLD)
            .child("✓");
    }
    tick.into_any_element()
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let close = if after.starts_with("<!--") {
            after.find("-->").map(|i| i + 3)
        } else {
            after.find('>').map(|i| i + 1)
        };
        match close {
            Some(end) => rest = &after[end..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}
