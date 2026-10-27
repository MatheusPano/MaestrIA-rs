mod buffers;
mod find_bar;
mod glyphs;
mod menu;
#[cfg(test)]
mod spec_tests;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
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
use crate::setup::editor::{EditorStyle, Gutter, Pad, SyntaxColors};

#[cfg(test)]
pub use buffers::{own_code_buffers_with, owners};
pub use buffers::own_code_buffers;
pub use find_bar::FindBar;
#[cfg(test)]
pub use find_bar::find_all;
pub use menu::{CodeMenu, menu_entries};

pub const PADDING: Pad = Pad { left: 6., top: 6., right: 10., bottom: 40. };

#[derive(Clone, Debug, PartialEq)]
pub struct CodeSpec {
    pub doc: String,
    pub text: Option<String>,
    pub revision: i64,
    pub language: Option<String>,
    pub line: Option<i64>,
    pub column: Option<i64>,
    pub jump: i64,
    pub read_only: bool,
    pub word_wrap: bool,
    pub line_numbers: bool,
    pub autofocus: bool,
    pub hint: Option<String>,
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
            line_numbers: e.line_numbers,
            autofocus: e.autofocus,
            hint: e.hint.clone(),
            font_size: e.font_size,
            on_change: owned(&e.on_change),
            on_save: owned(&e.on_save),
            on_cursor: owned(&e.on_cursor),
        }
    }
}

pub fn syntax_colors(theme: &Theme) -> SyntaxColors {
    let a = &theme.ansi;
    SyntaxColors {
        plain: hsla(theme.fg),
        comment: hsla(theme.fg_faint),
        keyword: hsla(a.magenta),
        kind: hsla(a.yellow),
        function: hsla(a.blue),
        string: hsla(a.green),
        number: hsla(a.cyan),
        attr: hsla(a.cyan),
        variable: hsla(a.red),
        meta: hsla(theme.fg_dim),
    }
}

pub fn editor_style(theme: &Theme, spec: &CodeSpec) -> EditorStyle {
    let size = spec.font_size as f32;
    let found = alpha(theme.yellow, 0.30);
    EditorStyle {
        font_size: size,
        line_height: 1.5,
        fg: hsla(theme.fg),
        caret: hsla(theme.accent),
        selection: alpha(theme.accent, 0.28),
        thumb: alpha(theme.fg_faint, 0.35),
        wrap: spec.word_wrap,
        code: true,
        language: spec.language.clone().unwrap_or_default(),
        syntax: Some(syntax_colors(theme)),
        gutter: spec.line_numbers.then(|| Gutter {
            color: hsla(theme.fg_faint),
            current: hsla(theme.fg_dim),
            font_size: size - 1.,
            fold: Some(hsla(theme.fg_faint)),
        }),
        found,
        found_current: found,
        current_line: Some(alpha(theme.fg, 0.045)),
        hint: spec.hint.clone().map(|h| (h, hsla(theme.fg_faint))),
        pad: PADDING,
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

type Shared = Rc<RefCell<Vec<Entity<CodePane>>>>;

#[derive(Default)]
pub struct CodeEditors {
    attached: HashMap<String, Entity<CodePane>>,
    watching: Option<Shared>,
}

impl CodeEditors {
    pub fn sync(&mut self, tree: &Widget, theme: &Arc<Theme>, sink: &ActionSink, window: &mut Window, cx: &mut Context<RfwView>) {
        let owner = buffers::owner_of(cx.entity_id(), cx);
        let shared = self.watch(window, cx);
        let mut seen = HashSet::new();
        for spec in specs(tree) {
            if !seen.insert(spec.doc.clone()) {
                continue;
            }
            let style = editor_style(theme, &spec);
            if let Some(pane) = self.attached.get(&spec.doc) {
                pane.update(cx, |p, cx| p.apply(spec, style, sink.clone(), window, cx));
                continue;
            }
            let pane = match buffers::pane(&owner, &spec.doc, cx) {
                Some(p) => p,
                None => {
                    let p = cx.new(|cx| CodePane::new(&spec, theme.clone(), window, cx));
                    buffers::keep(&owner, &spec.doc, p.clone(), cx);
                    p
                }
            };
            let doc = spec.doc.clone();
            pane.update(cx, |p, cx| p.attach(spec, style, sink.clone(), window, cx));
            self.attached.insert(doc, pane);
        }
        let gone: Vec<String> = self.attached.keys().filter(|d| !seen.contains(*d)).cloned().collect();
        for doc in gone {
            if let Some(pane) = self.attached.remove(&doc) {
                pane.update(cx, |p, cx| p.detach(window, cx));
            }
        }
        *shared.borrow_mut() = self.attached.values().cloned().collect();
    }

    fn watch(&mut self, window: &mut Window, cx: &mut Context<RfwView>) -> Shared {
        if let Some(s) = &self.watching {
            return s.clone();
        }
        let shared: Shared = Rc::default();
        let panes = shared.clone();
        let handle = window.window_handle();
        let view = cx.entity_id();
        cx.on_release(move |_, cx| {
            let panes = panes.take();
            cx.defer(move |cx| {
                handle
                    .update(cx, |_, window, cx| {
                        for p in &panes {
                            p.update(cx, |p, cx| p.detach(window, cx));
                        }
                    })
                    .ok();
                buffers::released(view, cx);
            });
        })
        .detach();
        self.watching = Some(shared.clone());
        shared
    }

    pub fn pane(&self, doc: &str) -> Option<&Entity<CodePane>> {
        self.attached.get(doc)
    }
}

pub fn render(cx: &mut Ctx, e: &CodeEditor) -> El {
    let el = cx.el().flex().flex_col().size_full().min_h(gpui::px(0.0)).min_w(gpui::px(0.0));
    match cx.editors.and_then(|m| m.pane(&e.doc)) {
        Some(pane) => el.child(pane.clone()),
        None => el,
    }
}
