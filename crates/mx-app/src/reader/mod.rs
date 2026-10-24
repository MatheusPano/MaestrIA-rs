mod blocks;
pub mod demo;
mod header;
mod inline;
mod style;
mod table;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chrono::Timelike;
use gpui::{
    App, ClipboardItem, Context, FocusHandle, Focusable, IntoElement, ParentElement, Pixels, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement, Styled, Task, Window, canvas, div, point,
    prelude::*, px,
};
use mx_claude::docs::{MxDoc, decode_href};
use mx_claude::markdown::{MdNode, parse_markdown};
use mx_theme::Theme;

pub use header::Action;

use blocks::Painter;
use header::{Glyph, HeaderButton, header, notice, source_glyph};
use crate::app::icons::Icon;
use inline::FollowHref;
use style::{SANS, Sheet};

const PULSE: Duration = Duration::from_millis(1500);
const COLUMN: f32 = 760.;

pub type PathAction = Rc<dyn Fn(String, &mut Window, &mut App)>;
pub type LinkHandler = Rc<dyn Fn(String, Option<String>, &mut Window, &mut App)>;

#[derive(Clone, Default)]
pub struct ReaderActions {
    pub follow: Option<LinkHandler>,
    pub banner: Option<PathAction>,
    pub reveal: Option<PathAction>,
    pub open_in_editor: Option<PathAction>,
    pub toggle_pin: Option<Action>,
    pub close: Option<Action>,
}

pub fn markdown_body(text: &str, theme: &Theme, zoom: f32, width: f32, window: &Window) -> gpui::AnyElement {
    let sheet = Sheet::new(theme, zoom);
    let tree = parse_markdown(text);
    let painter = Painter::new(&sheet, None, window.text_system().clone(), px(width));
    div()
        .w_full()
        .font_family(SANS)
        .text_color(sheet.fg)
        .child(painter.document(&tree))
        .into_any_element()
}

pub struct ReaderView {
    doc: MxDoc,
    tree: Rc<MdNode>,
    showing: String,
    theme: Arc<Theme>,
    zoom: f32,
    title: Option<String>,
    subtitle: Option<String>,
    cwd: Option<String>,
    home: Option<String>,
    pinned: bool,
    visible: bool,
    actions: ReaderActions,
    scroll: ScrollHandle,
    width: Rc<Cell<Pixels>>,
    poll: Option<Task<()>>,
    focus: FocusHandle,
}

impl Focusable for ReaderView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[allow(dead_code)]
impl ReaderView {
    pub fn new(doc: MxDoc, theme: Arc<Theme>, actions: ReaderActions, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            doc: MxDoc::new(doc.source, "", ""),
            tree: Rc::new(parse_markdown("")),
            showing: String::new(),
            theme,
            zoom: 0.,
            title: None,
            subtitle: None,
            cwd: None,
            home: std::env::var("HOME").ok(),
            pinned: false,
            visible: true,
            actions,
            scroll: ScrollHandle::new(),
            width: Rc::new(Cell::new(px(COLUMN))),
            poll: None,
            focus: cx.focus_handle(),
        };
        view.set_doc(doc, cx);
        view
    }

    pub fn doc(&self) -> &MxDoc {
        &self.doc
    }

    pub fn set_doc(&mut self, doc: MxDoc, cx: &mut Context<Self>) {
        let identity = doc.identity();
        self.doc = doc;
        if identity != self.showing {
            self.showing = identity;
            self.scroll.set_offset(point(px(0.), px(0.)));
            self.poll = None;
            self.doc.reload();
        }
        self.tree = Rc::new(parse_markdown(&self.doc.text));
        self.sync_poll(cx);
        cx.notify();
    }

    pub fn set_theme(&mut self, theme: Arc<Theme>, cx: &mut Context<Self>) {
        if !Arc::ptr_eq(&self.theme, &theme) {
            self.theme = theme;
            cx.notify();
        }
    }

    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        if self.zoom != zoom {
            self.zoom = zoom;
            cx.notify();
        }
    }

    pub fn set_header(&mut self, title: Option<String>, subtitle: Option<String>, cx: &mut Context<Self>) {
        if self.title != title || self.subtitle != subtitle {
            self.title = title;
            self.subtitle = subtitle;
            cx.notify();
        }
    }

    pub fn set_paths(&mut self, cwd: Option<String>, home: Option<String>) {
        self.cwd = cwd;
        self.home = home;
    }

    pub fn set_pinned(&mut self, pinned: bool, cx: &mut Context<Self>) {
        if self.pinned != pinned {
            self.pinned = pinned;
            cx.notify();
        }
    }

    pub fn set_actions(&mut self, actions: ReaderActions, cx: &mut Context<Self>) {
        self.actions = actions;
        cx.notify();
    }

    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible != visible {
            self.visible = visible;
            if visible {
                self.reload(cx);
            }
            self.sync_poll(cx);
        }
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if self.doc.reload() {
            self.tree = Rc::new(parse_markdown(&self.doc.text));
            cx.notify();
        }
    }

    fn sync_poll(&mut self, cx: &mut Context<Self>) {
        let wanted = self.visible && self.doc.source.on_disk() && self.doc.path.is_some();
        if !wanted {
            self.poll = None;
            return;
        }
        if self.poll.is_some() {
            return;
        }
        self.poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PULSE).await;
                let Ok(mut doc) = this.update(cx, |view, _| view.doc.clone()) else {
                    break;
                };
                let (changed, doc) = cx
                    .background_executor()
                    .spawn(async move { (doc.reload(), doc) })
                    .await;
                if changed {
                    let adopted = this.update(cx, |view, cx| view.adopt_reloaded(doc, cx));
                    if adopted.is_err() {
                        break;
                    }
                }
            }
        }));
    }

    fn adopt_reloaded(&mut self, doc: MxDoc, cx: &mut Context<Self>) {
        if doc.identity() == self.showing && doc.text != self.doc.text {
            self.doc.text = doc.text;
            self.doc.at = doc.at;
            self.tree = Rc::new(parse_markdown(&self.doc.text));
            cx.notify();
        }
    }

    fn title(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.doc.title.clone())
    }

    fn subtitle(&self) -> String {
        if let Some(subtitle) = &self.subtitle {
            return subtitle.clone();
        }
        let label = self.doc.source.label();
        if self.doc.missing() {
            return format!("{label} · o arquivo não está mais lá");
        }
        let when = format!("{:02}:{:02}", self.doc.at.hour(), self.doc.at.minute());
        let title = self.title();
        let mut parts = vec![label.to_owned()];
        if let Some(origin) = self.doc.origin.as_deref().filter(|o| !o.is_empty() && !title.contains(o)) {
            parts.push(format!("de {origin}"));
        }
        parts.push(when);
        parts.join(" · ")
    }

    fn follower(&self) -> Option<FollowHref> {
        let follow = self.actions.follow.clone()?;
        let base = match &self.doc.path {
            Some(path) => std::path::Path::new(path)
                .parent()
                .map(|p| p.to_string_lossy().into_owned()),
            None => self.cwd.clone(),
        };
        Some(Rc::new(move |href: &str, window: &mut Window, cx: &mut App| {
            follow(decode_href(href), base.clone(), window, cx);
        }))
    }

    fn buttons(&self, cx: &mut Context<Self>) -> Vec<HeaderButton> {
        let weak = cx.entity().downgrade();
        let mut buttons = Vec::new();
        if let Some(path) = self.doc.path.clone().filter(|_| self.doc.source.on_disk()) {
            let reload = weak.clone();
            buttons.push(HeaderButton {
                glyph: Glyph::Path("refresh_rounded"),
                tint: None,
                tooltip: "Reler do disco — ele já relê sozinho enquanto está aberto",
                action: Rc::new(move |_, cx| {
                    reload.update(cx, |view, cx| view.reload(cx)).ok();
                }),
            });
            for (glyph, tooltip, target) in [
                (Glyph::Path("folder_outlined"), "Mostrar no Finder", self.actions.reveal.clone()),
                (Glyph::Path("edit_outlined"), "Abrir no vscode", self.actions.open_in_editor.clone()),
            ] {
                if let Some(target) = target {
                    let path = path.clone();
                    buttons.push(HeaderButton {
                        glyph,
                        tint: None,
                        tooltip,
                        action: Rc::new(move |window, cx| target(path.clone(), window, cx)),
                    });
                }
            }
        }
        let copy = weak.clone();
        buttons.push(HeaderButton {
            glyph: Glyph::Path("copy_rounded"),
            tint: None,
            tooltip: "Copiar o markdown",
            action: Rc::new(move |window, cx| {
                copy.update(cx, |view, cx| view.copy(window, cx)).ok();
            }),
        });
        if let Some(pin) = self.actions.toggle_pin.clone() {
            buttons.push(HeaderButton {
                glyph: Glyph::Font(if self.pinned { Icon::PushPin } else { Icon::PushPinOutlined }),
                tint: self.pinned.then(|| style::hsla(self.theme.accent)),
                tooltip: if self.pinned { "Desafixar" } else { "Fixar" },
                action: pin,
            });
        }
        if let Some(close) = self.actions.close.clone() {
            buttons.push(HeaderButton {
                glyph: Glyph::Font(Icon::CloseRounded),
                tint: None,
                tooltip: "Fechar este leitor",
                action: close,
            });
        }
        buttons
    }

    fn copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.doc.text.clone()));
        if let Some(banner) = self.actions.banner.clone() {
            banner(format!("{}: markdown copiado", self.doc.title), window, cx);
        }
    }

    fn body(&self, sheet: &Sheet, window: &mut Window) -> gpui::AnyElement {
        if self.doc.missing() {
            let path = self.doc.path.clone().unwrap_or_default();
            return notice(sheet, Glyph::Path("error_outline"), "Esse arquivo não está mais lá", path);
        }
        if self.doc.text.trim().is_empty() {
            let title = if self.doc.source.on_disk() {
                "Arquivo vazio"
            } else {
                "Documento vazio"
            };
            return notice(sheet, Glyph::Path("description_outlined"), title, self.doc.path.clone().unwrap_or_default());
        }
        let painter = Painter::new(sheet, self.follower(), window.text_system().clone(), self.width.get());
        let width = self.width.clone();
        let probe = canvas(
            move |bounds, window, _| {
                if (bounds.size.width - width.get()).abs() > px(0.5) {
                    width.set(bounds.size.width);
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px(1.));
        div()
            .id("reader-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(
                div()
                    .w_full()
                    .pl(px(26.))
                    .pt(px(22.))
                    .pr(px(26.))
                    .pb(px(40.))
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .max_w(px(COLUMN))
                            .child(probe)
                            .child(painter.document(&self.tree)),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ReaderView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet = Sheet::new(&self.theme, self.zoom);
        let top = header(
            &sheet,
            source_glyph(self.doc.source),
            SharedString::from(self.title()),
            SharedString::from(self.subtitle()),
            self.buttons(cx),
        );
        let body = self.body(&sheet, window);
        div()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .font_family(SANS)
            .bg(sheet.bg)
            .text_color(sheet.fg)
            .child(top)
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{AppContext, TestAppContext};
    use mx_claude::docs::{DocSource, MxDoc};

    use super::{ReaderActions, ReaderView};

    #[gpui::test]
    fn resyncing_the_same_props_does_not_repaint(cx: &mut TestAppContext) {
        let theme = mx_theme::default_theme().clone();
        let doc = MxDoc::new(DocSource::Message, "Plano", "# oi");
        let view = cx.new(|cx| ReaderView::new(doc, theme.clone(), ReaderActions::default(), cx));
        let count = Rc::new(RefCell::new(0));
        let seen = count.clone();
        let _watch = cx.update(|cx| cx.observe(&view, move |_, _| *seen.borrow_mut() += 1));
        view.update(cx, |v, cx| {
            v.set_theme(theme.clone(), cx);
            v.set_zoom(0., cx);
            v.set_header(None, None, cx);
            v.set_pinned(false, cx);
        });
        cx.run_until_parked();
        assert_eq!(*count.borrow(), 0);
        view.update(cx, |v, cx| v.set_zoom(1., cx));
        cx.run_until_parked();
        assert_eq!(*count.borrow(), 1);
    }
}
