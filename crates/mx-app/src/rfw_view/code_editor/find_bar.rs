use std::ops::Range;

use gpui::{
    AppContext, BoxShadow, Context, Entity, Focusable, Hsla, InteractiveElement, KeyDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, Subscription, Window, div, point, px,
};
use mx_theme::Theme;

use super::glyphs::{Glyph, glyph};
use crate::rfw_view::code_pane::CodePane;
use crate::rfw_view::style::{UI_FONT, alpha, hsla};
use crate::rfw_view::text_input::{InputColors, TextInput};
use crate::rfw_view::tooltip;

const MAX_FOUND: usize = 10_000;
const ROW: f32 = 30.;
const GAP: f32 = 6.;
const WIDTH: f32 = 400.;

pub struct FindBar {
    pub query: Entity<TextInput>,
    pub replace: Option<Entity<TextInput>>,
    pub replacing: bool,
    pub case: bool,
    pub regex: bool,
    pub found: Vec<Range<usize>>,
    pub current: Option<usize>,
    _watch: Vec<Subscription>,
}

impl FindBar {
    pub fn count(&self, cx: &gpui::App) -> (String, bool) {
        if self.query.read(cx).text.is_empty() {
            return (String::new(), false);
        }
        match self.found.len() {
            0 => ("nenhum".to_owned(), true),
            n => (format!("{} de {n}", self.current.unwrap_or(0) + 1), false),
        }
    }
}

pub fn find_all(text: &str, query: &str, case: bool, regex: bool) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let pattern = if regex { query.to_owned() } else { regex::escape(query) };
    let Ok(re) = regex::RegexBuilder::new(&pattern).case_insensitive(!case).multi_line(true).build() else {
        return Vec::new();
    };
    re.find_iter(text).filter(|m| !m.is_empty()).take(MAX_FOUND).map(|m| m.range()).collect()
}

fn input(text: String, placeholder: &str, p: &Theme, cx: &mut Context<CodePane>) -> Entity<TextInput> {
    let colors = InputColors { fg: hsla(p.fg), faint: hsla(p.fg_faint), caret: hsla(p.accent) };
    let placeholder = placeholder.to_owned();
    cx.new(|cx| {
        let mut t = TextInput::new(text, colors, cx);
        t.placeholder = Some(placeholder);
        t.size = 12.5;
        t
    })
}

impl CodePane {
    pub fn open_find(&mut self, with_replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        let picked = self.editor.read(cx).selected_text().to_owned();
        let seed = (!picked.is_empty() && !picked.contains('\n')).then_some(picked);
        let theme = self.theme.clone();
        match &self.find {
            None => {
                let query = input(seed.unwrap_or_default(), "buscar", &theme, cx);
                let watch = vec![cx.observe(&query, |this, _, cx| this.refind(cx))];
                self.find = Some(FindBar {
                    query,
                    replace: None,
                    replacing: false,
                    case: false,
                    regex: false,
                    found: Vec::new(),
                    current: None,
                    _watch: watch,
                });
            }
            Some(bar) => {
                if let Some(s) = seed {
                    bar.query.update(cx, |q, cx| {
                        q.text = s;
                        cx.notify();
                    });
                }
            }
        }
        if with_replace && !self.spec.read_only {
            self.show_replace(true, cx);
        }
        if let Some(bar) = &self.find {
            window.focus(&bar.query.read(cx).focus_handle(cx));
        }
        self.refind(cx);
        cx.notify();
    }

    fn show_replace(&mut self, on: bool, cx: &mut Context<Self>) {
        let theme = self.theme.clone();
        let needs = on && self.find.as_ref().is_some_and(|b| b.replace.is_none());
        let fresh = needs.then(|| input(String::new(), "trocar por", &theme, cx));
        if let Some(bar) = &mut self.find {
            if fresh.is_some() {
                bar.replace = fresh;
            }
            bar.replacing = on;
        }
        cx.notify();
    }

    pub fn toggle_replace(&mut self, cx: &mut Context<Self>) {
        if self.spec.read_only {
            return;
        }
        let on = !self.find.as_ref().is_some_and(|b| b.replacing);
        self.show_replace(on, cx);
    }

    pub fn toggle_case(&mut self, cx: &mut Context<Self>) {
        if let Some(bar) = &mut self.find {
            bar.case = !bar.case;
        }
        self.refind(cx);
    }

    pub fn toggle_regex(&mut self, cx: &mut Context<Self>) {
        if let Some(bar) = &mut self.find {
            bar.regex = !bar.regex;
        }
        self.refind(cx);
    }

    pub fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        self.editor.update(cx, |e, cx| e.set_found(Vec::new(), None, cx));
        self.editor.read(cx).focus(window);
        cx.notify();
    }

    pub fn refind(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &mut self.find else { return };
        let query = bar.query.read(cx).text.clone();
        let editor = self.editor.read(cx);
        let found = find_all(editor.text(), &query, bar.case, bar.regex);
        let from = editor.selection().start;
        let current = (!found.is_empty()).then(|| found.iter().position(|r| r.start >= from).unwrap_or(0));
        bar.found = found.clone();
        bar.current = current;
        self.editor.update(cx, |e, cx| e.set_found(found, None, cx));
        cx.notify();
    }

    pub fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(bar) = &mut self.find else { return };
        let n = bar.found.len();
        if n == 0 {
            return;
        }
        let sel = self.editor.read(cx).selection();
        let next = match bar.current {
            Some(i) if bar.found.get(i) == Some(&sel) => {
                if forward {
                    (i + 1) % n
                } else {
                    (i + n - 1) % n
                }
            }
            Some(i) => i,
            None => 0,
        };
        bar.current = Some(next);
        let found = bar.found.clone();
        self.editor.update(cx, |e, cx| e.set_found(found, Some(next), cx));
        cx.notify();
    }

    fn replacement(&self, cx: &Context<Self>) -> String {
        let bar = self.find.as_ref();
        bar.and_then(|b| b.replace.as_ref()).map(|r| r.read(cx).text.clone()).unwrap_or_default()
    }

    pub fn replace_one(&mut self, cx: &mut Context<Self>) {
        let with = self.replacement(cx);
        let Some(bar) = &self.find else { return };
        let sel = self.editor.read(cx).selection();
        if bar.current.and_then(|i| bar.found.get(i)) == Some(&sel) {
            self.editor.update(cx, |e, cx| e.replace_range(sel, &with, cx));
            self.refind(cx);
        }
        self.step(true, cx);
    }

    pub fn replace_every(&mut self, cx: &mut Context<Self>) {
        let with = self.replacement(cx);
        let Some(bar) = &self.find else { return };
        let found = bar.found.clone();
        self.editor.update(cx, |e, cx| e.replace_all(&found, &with, cx));
        self.refind(cx);
    }

    #[cfg(test)]
    pub fn replace_every_with(&mut self, with: &str, cx: &mut Context<Self>) {
        if let Some(r) = self.find.as_ref().and_then(|b| b.replace.clone()) {
            r.update(cx, |r, _| r.text = with.to_owned());
        }
        self.replace_every(cx);
    }

    fn bar_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        if k.key != "enter" {
            return;
        }
        let in_replace = self.find.as_ref().and_then(|b| b.replace.as_ref()).is_some_and(|r| r.read(cx).focused(window));
        if in_replace {
            self.replace_one(cx);
        } else {
            self.step(!k.modifiers.shift, cx);
        }
        cx.stop_propagation();
    }
}

fn icon_button(id: &'static str, g: Glyph, tip: &str, enabled: bool, p: &Theme) -> gpui::Stateful<gpui::Div> {
    let color = if enabled { hsla(p.fg_dim) } else { hsla(p.fg_faint) };
    let hover = hsla(p.bg_hover);
    let mut b = div()
        .id(id)
        .flex_none()
        .size(px(26.))
        .rounded(px(5.))
        .flex()
        .items_center()
        .justify_center()
        .tooltip(tooltip::builder(tip.to_owned(), p))
        .child(glyph(g, 16., color));
    if enabled {
        b = b.cursor_pointer().hover(move |s| s.bg(hover));
    }
    b
}

fn toggle(id: &'static str, label: &'static str, tip: &str, on: bool, p: &Theme) -> gpui::Stateful<gpui::Div> {
    let mut t = div()
        .id(id)
        .flex_none()
        .w(px(24.))
        .h(px(22.))
        .ml(px(2.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .font_family(crate::rfw_view::view::mono_family())
        .text_size(px(11.))
        .tooltip(tooltip::builder(tip.to_owned(), p))
        .child(label);
    if on {
        t = t.bg(alpha(p.accent, 0.18)).border_1().border_color(alpha(p.accent, 0.6)).text_color(hsla(p.accent));
    } else {
        t = t.text_color(hsla(p.fg_dim));
    }
    t
}

fn field(input: &Entity<TextInput>, p: &Theme) -> gpui::Div {
    div()
        .flex_1()
        .min_w(px(0.))
        .h(px(ROW))
        .pl(px(8.))
        .pr(px(2.))
        .flex()
        .flex_row()
        .items_center()
        .rounded(px(5.))
        .border_1()
        .border_color(hsla(p.border))
        .bg(hsla(p.bg))
        .child(input.clone())
}

fn shadow(color: Hsla) -> Vec<BoxShadow> {
    vec![BoxShadow { color, offset: point(px(0.), px(4.)), blur_radius: px(14.), spread_radius: px(0.) }]
}

impl CodePane {
    pub fn render_bar(&self, cx: &mut Context<Self>) -> Option<gpui::Stateful<gpui::Div>> {
        let bar = self.find.as_ref()?;
        let p = self.theme.clone();
        let read_only = self.spec.read_only;
        let replacing = bar.replacing && !read_only;
        let (count, none) = bar.count(cx);
        let mode = if replacing { Glyph::ExpandMore } else { Glyph::ChevronRight };
        let query = field(&bar.query, &p)
            .child(toggle("find-case", "Aa", "maiúsculas", bar.case, &p).on_click(cx.listener(|this, _, _, cx| this.toggle_case(cx))))
            .child(toggle("find-regex", ".*", "expressão regular", bar.regex, &p).on_click(cx.listener(|this, _, _, cx| this.toggle_regex(cx))));
        let counter = div()
            .flex_none()
            .w(px(64.))
            .flex()
            .justify_center()
            .text_size(px(11.5))
            .text_color(if none { hsla(p.red) } else { hsla(p.fg_dim) })
            .child(count);
        let first = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(ROW))
            .child(icon_button("find-mode", mode, "trocar", !read_only, &p).on_click(cx.listener(|this, _, _, cx| this.toggle_replace(cx))))
            .child(query)
            .child(counter)
            .child(icon_button("find-prev", Glyph::ArrowUp, "anterior", true, &p).on_click(cx.listener(|this, _, _, cx| this.step(false, cx))))
            .child(icon_button("find-next", Glyph::ArrowDown, "próximo", true, &p).on_click(cx.listener(|this, _, _, cx| this.step(true, cx))))
            .child(
                icon_button("find-close", Glyph::Close, "fechar", true, &p)
                    .on_click(cx.listener(|this, _, window, cx| this.close_find(window, cx))),
            );
        let second = bar.replace.as_ref().filter(|_| replacing).map(|r| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .h(px(ROW))
                .mt(px(4.))
                .child(div().flex_none().w(px(26.)))
                .child(field(r, &p))
                .child(div().flex_none().w(px(4.)))
                .child(
                    icon_button("replace-one", Glyph::FindReplace, "trocar este", true, &p)
                        .on_click(cx.listener(|this, _, _, cx| this.replace_one(cx))),
                )
                .child(
                    icon_button("replace-all", Glyph::DoneAll, "trocar todos", true, &p)
                        .on_click(cx.listener(|this, _, _, cx| this.replace_every(cx))),
                )
                .child(div().flex_none().w(px(52.)))
        });
        Some(
            div()
                .id("find-bar")
                .absolute()
                .top(px(GAP))
                .right(px(14.))
                .w(px(WIDTH))
                .p(px(4.))
                .flex()
                .flex_col()
                .bg(hsla(p.bg_sidebar))
                .rounded(px(8.))
                .border_1()
                .border_color(hsla(p.border))
                .shadow(shadow(hsla(p.shadow())))
                .font_family(UI_FONT)
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .capture_key_down(cx.listener(Self::bar_key))
                .child(first)
                .children(second),
        )
    }
}
