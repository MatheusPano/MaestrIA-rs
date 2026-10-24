use std::path::PathBuf;
use std::sync::Arc;

use gpui::{Context, Div, IntoElement, ParentElement, Render, Styled, Window, div, px, relative};
use mx_rfw::{DynamicContent, Library, evaluate, parse_error_text, parse_library_file};
use mx_theme::{Palette, Theme, Typography};

use super::ActionSink;
use super::code_editor::CodeEditors;
use super::layout::{Ctx, DragNow, Fit, grow};
use super::menu::{self, HasMenu, OpenMenu};
use super::style::{UI_FONT, UI_LINE, hsla, rfw_theme};
use super::widgets_core::render;

pub fn mono_family() -> &'static str {
    "Hack"
}

pub fn text_root(d: Div, p: &Palette) -> Div {
    d.text_color(hsla(p.fg)).text_size(px(13.0)).font_family(UI_FONT).line_height(relative(UI_LINE))
}

pub struct RfwView {
    source: Option<String>,
    library: Option<Library>,
    error: Option<String>,
    root: String,
    content: DynamicContent,
    theme: Arc<Theme>,
    dir: Option<PathBuf>,
    sink: ActionSink,
    background: Option<gpui::Hsla>,
    pub(super) menu: Option<OpenMenu>,
    hoisted_menu: bool,
    pub(super) dragging: Option<DragNow>,
    editors: CodeEditors,
}

impl HasMenu for RfwView {
    fn menu_slot(&mut self) -> &mut Option<OpenMenu> {
        &mut self.menu
    }
}

impl RfwView {
    pub fn new(theme: Arc<Theme>, typography: &Typography, dir: Option<PathBuf>, sink: ActionSink) -> Self {
        let mut content = DynamicContent::new();
        content.set_theme(rfw_theme(&theme, typography));
        Self {
            source: None,
            library: None,
            error: None,
            root: "root".into(),
            content,
            theme,
            dir,
            sink,
            background: None,
            menu: None,
            hoisted_menu: false,
            dragging: None,
            editors: CodeEditors::default(),
        }
    }

    pub fn set_library(&mut self, text: &str) {
        if self.source.as_deref() == Some(text) {
            return;
        }
        self.source = Some(text.to_owned());
        match parse_library_file(text) {
            Ok(lib) => {
                self.library = Some(lib);
                self.error = None;
            }
            Err(e) => self.error = Some(parse_error_text(&e)),
        }
    }

    pub fn hoist_menu(&mut self) {
        self.hoisted_menu = true;
    }

    pub fn menu_overlay(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let theme = self.theme.clone();
        self.menu.as_ref().map(|m| menu::floating(&theme, cx, m).into_any_element())
    }

    pub fn set_root(&mut self, root: &str) {
        root.clone_into(&mut self.root);
    }

    pub fn merge_data(&mut self, data: &serde_json::Map<String, serde_json::Value>) {
        self.content.merge_plugin_data(data);
    }

    pub fn set_theme(&mut self, theme: Arc<Theme>, typography: &Typography) {
        self.content.set_theme(rfw_theme(&theme, typography));
        self.theme = theme;
    }

    pub fn set_dir(&mut self, dir: Option<PathBuf>) {
        self.dir = dir;
    }

    pub(super) fn code_pane(&self, doc: &str) -> Option<gpui::Entity<super::code_pane::CodePane>> {
        self.editors.pane(doc).cloned()
    }

    pub fn set_background(&mut self, background: Option<gpui::Hsla>) {
        self.background = background;
    }
}

impl Render for RfwView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !cx.has_active_drag() {
            self.dragging = None;
        }
        let p = self.theme.clone();
        let mut root = text_root(div(), &p).size_full().flex().flex_col();
        if let Some(bg) = self.background {
            root = root.bg(bg);
        }
        if let Some(e) = &self.error {
            return root
                .p(px(14.0))
                .text_size(px(11.5))
                .line_height(relative(1.45))
                .font_family(mono_family())
                .text_color(hsla(p.red))
                .child(e.clone())
                .into_any_element();
        }
        let Some(lib) = &self.library else { return root.into_any_element() };
        let tree = evaluate(lib, &self.root, &self.content);
        self.editors.sync(&tree, &self.theme, &self.sink, window, cx);
        let mut ctx = Ctx {
            palette: &self.theme,
            dir: self.dir.as_deref(),
            sink: self.sink.clone(),
            view: cx.weak_entity(),
            ids: 0,
            drops: Vec::new(),
            dragging: self.dragging.clone(),
            turns: 0.0,
            interactive: true,
            bounded_w: true,
            editors: Some(&self.editors),
        };
        let body = grow(render(&mut ctx, &tree, Fit::BOTH), 1.0);
        let overlay = self.menu.as_ref().filter(|_| !self.hoisted_menu).map(|m| menu::render(&p, cx, m));
        root.child(body).children(overlay).into_any_element()
    }
}
