use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use mx_plugins::ui::blocks::{Avatar, Block, ListItem};

use super::blocks::{BlocksView, Paint};
use super::blocks_button::{menu_rows, on_act, open_menu, tone};
use super::blocks_misc::blend;
use super::icons::{glyph, named, plugin_icon, svg_element};
use super::style::{accent_is_dark, alpha, hsla};
use super::tooltip;

pub fn avatar(p: &Paint, a: &Avatar, size: f32, dim: bool, status: Option<&str>) -> AnyElement {
    let th = &p.view.theme;
    let base = if a.color.is_none() { hsla(th.accent) } else { tone(p, a.color.as_deref()) };
    let color = if dim { hsla(th.fg_faint) } else { base };
    let dir = p.view.dir.as_deref();
    let boxed = if let Some(svg) = &a.svg {
        let pic = div().size(px(size)).child(svg_element(svg, Some(size), Some(size)));
        if dim { pic.opacity(0.7) } else { pic }
    } else if a.plain {
        div().size(px(size)).flex().items_center().justify_center().child(glyph(a.icon.as_deref(), dir, size * 0.8, color, 0.0))
    } else {
        let mut b = div()
            .size(px(size))
            .flex()
            .items_center()
            .justify_center()
            .bg(with_alpha(color, if dim { 0.10 } else { 0.17 }))
            .border_1()
            .border_color(with_alpha(color, if dim { 0.14 } else { 0.26 }));
        b = if a.circle { b.rounded_full() } else { b.rounded(px(size * 0.28)) };
        match &a.text {
            Some(t) => b.text_size(px(size * 0.4)).font_weight(FontWeight::BOLD).text_color(color).child(t.clone()),
            None => b.child(glyph(a.icon.as_deref(), dir, size * 0.56, color, 0.0)),
        }
    };
    let Some(status) = status else { return boxed.flex_none().into_any_element() };
    let dot = if size < 32.0 { 9.0 } else { 11.0 };
    let off = if a.circle || a.svg.is_some() { -1.0 } else { -2.5 };
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(boxed)
        .child(
            div()
                .absolute()
                .right(px(off))
                .bottom(px(off))
                .size(px(dot))
                .rounded_full()
                .bg(tone(p, Some(status)))
                .border_2()
                .border_color(p.surface),
        )
        .into_any_element()
}

fn with_alpha(c: Hsla, a: f32) -> Hsla {
    Hsla { a: c.a * a, ..c }
}

fn row(p: &mut Paint, it: &ListItem, tree: bool, flat: bool, always: bool, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = p.view.theme.clone();
    let s = p.size();
    let clickable = it.action.is_some();
    let on_accent = hsla(if accent_is_dark(&th) { mx_theme::Color::WHITE } else { th.canvas });
    let surface = if it.selected { blend(alpha(th.accent, 0.85), p.surface) } else { p.surface };
    let saved = p.surface;
    p.surface = surface;
    let mut el = div().id(p.id("row")).flex().flex_row().items_center().group("");
    el = if flat {
        el.rounded(px(7.0)).pl(px(6.0 + it.indent as f32 * 16.0)).pr(px(6.0)).py(px(5.0))
    } else {
        el.pl(px(12.0 + it.indent as f32 * 18.0)).pr(px(12.0)).py(px(8.0))
    };
    if it.selected {
        let (bg, hover) = (alpha(th.accent, 0.85), alpha(th.accent, 0.95));
        el = el.bg(bg);
        if clickable {
            el = el.hover(move |s| s.bg(hover));
        }
    } else if clickable {
        let hover = hsla(th.bg_hover);
        el = el.hover(move |s| s.bg(hover));
    }
    if let Some(expanded) = it.expanded {
        let color = if it.selected { on_accent } else { hsla(th.fg_faint) };
        let mut chevron = div().id(p.id("chev")).flex_none().pr(px(3.0)).child(super::icons::named_turned("chevron_right_rounded", 18.0, color, if expanded { 0.25 } else { 0.0 }));
        if let Some(t) = &it.toggle {
            chevron = chevron.cursor_pointer().on_click(on_act(t.clone(), cx));
        }
        el = el.child(chevron);
    } else if tree {
        el = el.child(div().w(px(21.0)).flex_none());
    }
    if let Some(a) = &it.avatar {
        el = el.child(avatar(p, a, if flat { 26.0 } else { 30.0 }, it.dim, it.status.as_deref())).child(div().w(px(10.0)).flex_none());
    } else if let Some(icon) = plugin_icon(it.icon.as_deref()) {
        el = el.child(named(icon, 16.0, tone(p, it.tone.as_deref()))).child(div().w(px(10.0)).flex_none());
    }
    let title_color = if it.selected { on_accent } else if it.dim { hsla(th.fg_dim) } else { hsla(th.fg) };
    let mut title = div()
        .text_size(px(s - 0.5))
        .font_weight(if it.strong { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
        .text_color(title_color);
    if flat {
        title = title.truncate();
    }
    let mut col = div().flex_1().min_w(px(0.0)).flex().flex_col().child(title.child(it.title.clone()));
    if let Some(sub) = it.subtitle.as_ref().filter(|s| !s.is_empty()) {
        let color = if it.selected {
            alpha_h(on_accent, 0.75)
        } else if it.subtitle_tone.is_some() {
            tone(p, it.subtitle_tone.as_deref())
        } else if it.dim {
            hsla(th.fg_faint)
        } else {
            hsla(th.fg_dim)
        };
        let mut st = div().pt(px(1.5)).text_size(px(s - 2.0)).text_color(color);
        if flat {
            st = st.truncate();
        }
        col = col.child(st.child(sub.clone()));
    }
    el = el.child(col);
    let fg = th.fg;
    if !it.actions.is_empty() {
        let mut acts = div().flex().flex_row().flex_none();
        if !always {
            acts = acts.invisible().group_hover("", |s| s.visible());
        }
        for a in &it.actions {
            let color = if it.selected {
                on_accent
            } else if a.tone.is_none() {
                hsla(if it.dim { th.fg_faint } else { th.fg_dim })
            } else {
                tone(p, a.tone.as_deref())
            };
            let mut b = div()
                .id(p.id("act"))
                .size(px(26.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .cursor_pointer()
                .hover(move |s| s.bg(alpha(fg, 0.08)))
                .child(named(plugin_icon(Some(&a.icon)).unwrap_or("circle"), 16.0, color))
                .on_click(on_act(a.action.clone(), cx));
            if let Some(t) = &a.tooltip {
                b = b.tooltip(tooltip::builder(t.clone(), &th));
            }
            acts = acts.child(b);
        }
        el = el.child(acts);
    }
    if let Some(meta) = it.meta.as_ref().filter(|m| !m.is_empty() && (it.actions.is_empty() || !always)) {
        let color = if it.meta_tone.is_none() { hsla(th.fg_faint) } else { tone(p, it.meta_tone.as_deref()) };
        let mut m = div().pl(px(8.0)).text_size(px(s - 2.0)).text_color(color).child(meta.clone());
        if !it.actions.is_empty() {
            m = m.group_hover("", |s| s.invisible());
        }
        el = el.child(m);
    }
    if let Some(badge) = &it.badge {
        el = el.child(
            div()
                .ml(px(8.0))
                .px(px(7.0))
                .py(px(2.0))
                .bg(hsla(th.bg_active))
                .rounded(px(10.0))
                .text_size(px(11.0))
                .text_color(hsla(th.fg_dim))
                .child(badge.clone()),
        );
    }
    if let Some(a) = &it.action {
        el = el.cursor_pointer().on_click(on_act(a.clone(), cx));
    }
    if !it.menu.is_empty() {
        let open = open_menu(menu_rows(p, &it.menu), cx);
        el = el.on_mouse_down(MouseButton::Right, move |e, _, app| {
            app.stop_propagation();
            open(e.position, app);
        });
    }
    p.surface = saved;
    el.into_any_element()
}

fn alpha_h(c: Hsla, a: f32) -> Hsla {
    with_alpha(c, a)
}

pub fn list(p: &mut Paint, b: &Block, cx: &mut Context<BlocksView>) -> AnyElement {
    let Block::List { items, empty, flat, always_actions } = b else { return div().into_any_element() };
    let th = p.view.theme.clone();
    if items.is_empty() {
        return div().text_size(px(p.size() - 1.0)).text_color(hsla(th.fg_faint)).child(empty.clone()).into_any_element();
    }
    let tree = items.iter().any(|i| i.expanded.is_some());
    let mut col = div().flex().flex_col();
    if !*flat {
        col = col.rounded(px(8.0)).border_1().border_color(hsla(th.border)).overflow_hidden();
    }
    for (i, it) in items.iter().enumerate() {
        if i > 0 && !*flat {
            col = col.child(div().h(px(1.0)).bg(hsla(th.border)));
        }
        col = col.child(row(p, it, tree, *flat, *always_actions, cx));
    }
    col.into_any_element()
}
