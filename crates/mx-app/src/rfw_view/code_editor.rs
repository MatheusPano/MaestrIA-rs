use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{AppContext, Context, Entity, ParentElement, Styled, Window};
use mx_rfw::widgets::code_editor::CodeEditor;
use mx_rfw::{Event, LocalLibrary, Widget};
use mx_theme::Theme;

use super::ActionSink;
use super::code_pane::CodePane;
use super::layout::{Ctx, El, owned};
use super::style::{alpha, hsla};
use super::view::RfwView;
use crate::setup::editor::{EditorStyle, SyntaxColors};

#[derive(Clone, Debug, PartialEq)]
pub struct CodeSpec {
    pub doc: String,
    pub text: Option<String>,
    pub revision: i64,
    pub language: String,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub jump: i64,
    pub read_only: bool,
    pub word_wrap: bool,
    pub font_size: f64,
    pub on_change: Vec<Event>,
    pub on_save: Vec<Event>,
    pub on_cursor: Vec<Event>,
}

impl CodeSpec {
    pub fn of(e: &CodeEditor) -> Self {
        Self {
            doc: e.doc.clone(),
            text: e.text.clone(),
            revision: e.revision,
            language: e.language.clone(),
            line: e.line,
            column: e.column,
            jump: e.jump,
            read_only: e.read_only,
            word_wrap: e.word_wrap,
            font_size: e.font_size,
            on_change: owned(&e.on_change),
            on_save: owned(&e.on_save),
            on_cursor: owned(&e.on_cursor),
        }
    }
}

pub fn editor_style(theme: &Theme, spec: &CodeSpec) -> EditorStyle {
    let a = &theme.ansi;
    EditorStyle {
        font_size: spec.font_size as f32,
        line_height: 1.5,
        fg: hsla(theme.fg),
        caret: hsla(theme.accent),
        selection: alpha(theme.accent, 0.3),
        thumb: alpha(theme.fg_faint, 0.35),
        wrap: spec.word_wrap,
        code: true,
        language: spec.language.clone(),
        syntax: Some(SyntaxColors {
            comment: hsla(theme.fg_faint),
            string: hsla(theme.green),
            number: hsla(theme.yellow),
            keyword: hsla(theme.purple),
            kind: hsla(a.cyan),
            key: hsla(a.blue),
            meta: hsla(theme.accent),
            added: hsla(theme.green),
            removed: hsla(theme.red),
        }),
        gutter: Some((alpha(theme.fg_faint, 0.7), hsla(theme.fg_dim))),
        found: alpha(theme.yellow, 0.22),
        found_current: alpha(theme.yellow, 0.5),
    }
}

fn specs(tree: &Widget) -> Vec<CodeSpec> {
    let mut out = Vec::new();
    tree.walk(&mut |w| {
        if let Widget::Node(n) = w
            && n.library == LocalLibrary::Maestria
            && n.name == "CodeEditor"
        {
            out.push(CodeSpec::of(&CodeEditor::from_node(n)));
        }
    });
    out
}

#[derive(Default)]
pub struct CodeEditors {
    panes: HashMap<String, Entity<CodePane>>,
    shown: HashSet<String>,
}

impl CodeEditors {
    pub fn sync(&mut self, tree: &Widget, theme: &Arc<Theme>, sink: &ActionSink, window: &mut Window, cx: &mut Context<RfwView>) {
        let mut shown = HashSet::new();
        for spec in specs(tree) {
            let style = editor_style(theme, &spec);
            let reappeared = !self.shown.contains(&spec.doc);
            shown.insert(spec.doc.clone());
            match self.panes.get(&spec.doc) {
                Some(pane) => pane.update(cx, |p, cx| p.apply(spec, style, sink.clone(), reappeared, window, cx)),
                None => {
                    let doc = spec.doc.clone();
                    let sink = sink.clone();
                    let pane = cx.new(|cx| CodePane::new(spec, style, sink, theme.clone(), window, cx));
                    self.panes.insert(doc, pane);
                }
            }
        }
        self.shown = shown;
    }

    pub fn pane(&self, doc: &str) -> Option<&Entity<CodePane>> {
        self.panes.get(doc)
    }
}

pub fn render(cx: &mut Ctx, e: &CodeEditor) -> El {
    let el = cx.el().flex().flex_col().size_full().min_h(gpui::px(0.0)).min_w(gpui::px(0.0));
    match cx.editors.and_then(|m| m.pane(&e.doc)) {
        Some(pane) => el.child(pane.clone()),
        None => el,
    }
}
