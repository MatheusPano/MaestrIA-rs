use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, point, px, relative,
};
use mx_plugins::ui::blocks::{Block, TabItem};

use super::blocks::{BlocksView, Paint, block};
use super::blocks_button::{button, on_act, tone};
use super::blocks_list::avatar;
use super::blocks_text::{console, markdown};
use super::icons::{named, plugin_icon};
use super::style::{alpha, hsla};
use super::view::mono_family;

fn text_block(p: &Paint, text: &str, style: Option<&str>, center: bool) -> AnyElement {
    let th = &p.view.theme;
    let s = p.size();
    let mut el = div().text_size(px(s)).line_height(relative(1.5)).text_color(hsla(th.fg));
    el = match style {
        Some("dim") => el.text_color(hsla(th.fg_dim)),
        Some("faint") => el.text_color(hsla(th.fg_faint)).text_size(px(s - 1.0)),
        Some("mono") => el.font_family(mono_family()).text_size(px(s - 1.5)),
        Some("error") => el.text_color(hsla(th.red)),
        Some("success") => el.text_color(hsla(th.green)),
        Some("warning") => el.text_color(hsla(th.yellow)),
        _ => el,
    };
    if center {
        el = el.text_center();
    }
    el.child(text.to_owned()).into_any_element()
}

fn section(p: &mut Paint, text: &str, count: Option<&str>, collapsed: Option<bool>, action: Option<&String>, label: bool, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = &p.view.theme;
    let faint = hsla(th.fg_faint);
    if label {
        let s = p.size() - 1.5;
        return div().pt(px(10.0)).px(px(6.0)).text_size(px(s)).font_weight(FontWeight::SEMIBOLD).text_color(faint).child(text.to_owned()).into_any_element();
    }
    let mut row = div().id(p.id("section")).pt(px(4.0)).flex().flex_row().items_center();
    if let Some(c) = collapsed {
        row = row.child(named(if c { "chevron_right" } else { "expand_more" }, 14.0, faint)).child(div().w(px(2.0)));
    }
    row = row
        .child(div().text_size(px(10.5)).text_color(faint).child(text.to_owned()))
        .child(div().w(px(9.0)))
        .child(div().flex_1().h(px(1.0)).bg(hsla(th.border)));
    if let Some(c) = count {
        row = row.child(div().pl(px(8.0)).text_size(px(11.0)).text_color(faint).child(c.to_owned()));
    }
    if let Some(a) = action {
        row = row.cursor_pointer().on_click(on_act(a.clone(), cx));
    }
    row.into_any_element()
}

fn header(p: &mut Paint, b: &Block, cx: &mut Context<BlocksView>) -> AnyElement {
    let Block::Header { title, subtitle, tone: t, avatar: av, status, dim, wrap, actions } = b else { unreachable!() };
    let s = p.size();
    let bare = p.bare();
    let th = p.view.theme.clone();
    let mut row = div().flex().flex_row().items_center();
    if let Some(a) = av {
        row = row.child(avatar(p, a, if bare { 30.0 } else { 40.0 }, *dim, status.as_deref())).child(div().w(px(if bare { 10.0 } else { 14.0 })).flex_none());
    }
    let title_el = div().text_size(px(s + if bare { 2.5 } else { 5.0 })).font_weight(FontWeight::BOLD).text_color(hsla(th.fg));
    let title_el = if *wrap { title_el.line_clamp(3).text_ellipsis() } else { title_el.truncate() };
    let mut col = div().flex_1().min_w(px(0.0)).flex().flex_col().items_start().child(title_el.child(title.clone()));
    if let Some(sub) = subtitle.as_ref().filter(|s| !s.is_empty()) {
        let color = if t.is_none() { hsla(th.fg_dim) } else { tone(p, t.as_deref()) };
        col = col.child(div().pt(px(2.0)).text_size(px(s - 1.5)).text_color(color).truncate().max_w_full().child(sub.clone()));
    }
    row = row.child(col);
    for a in actions {
        row = row.child(button(p, a, cx));
    }
    row.into_any_element()
}

fn card(p: &mut Paint, children: &[Block], padding: Option<f64>, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = p.view.theme.clone();
    let tint = alpha(th.fg, if th.dark { 0.035 } else { 0.025 });
    let saved = p.surface;
    p.surface = blend(tint, saved);
    let mut col = div().flex().flex_col();
    for (i, c) in children.iter().enumerate() {
        col = col.child(block(p, c, i + 1 == children.len(), cx));
    }
    let surface = p.surface;
    p.surface = saved;
    let pad = padding.map_or(if p.bare() { 6.0 } else { 12.0 }, |v| v as f32);
    div().p(px(pad)).bg(surface).rounded(px(10.0)).border_1().border_color(hsla(th.border)).child(col).into_any_element()
}

pub fn blend(top: Hsla, under: Hsla) -> Hsla {
    let (t, u): (gpui::Rgba, gpui::Rgba) = (top.into(), under.into());
    let a = t.a;
    gpui::Rgba { r: t.r * a + u.r * (1.0 - a), g: t.g * a + u.g * (1.0 - a), b: t.b * a + u.b * (1.0 - a), a: u.a }.into()
}

fn tabs(p: &mut Paint, items: &[TabItem], center: bool, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = p.view.theme.clone();
    let s = p.size();
    let mut bar = div().p(px(3.0)).flex().flex_row().bg(alpha(th.bg_active, 0.7)).rounded(px(9.0));
    for it in items {
        let color = hsla(if it.active { th.fg } else { th.fg_dim });
        let hover = hsla(th.fg);
        let mut chip = div()
            .id(p.id("tab"))
            .px(px(12.0))
            .py(px(5.0))
            .rounded(px(7.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .text_color(color)
            .hover(move |st| st.text_color(hover))
            .children(plugin_icon(it.icon.as_deref()).map(|i| named(i, 14.0, color)))
            .child(div().text_size(px(s - 1.0)).font_weight(if it.active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM }).child(it.label.clone()))
            .children(it.count.as_ref().map(|c| div().text_size(px(s - 2.0)).text_color(hsla(th.fg_faint)).child(c.clone())));
        if it.active {
            chip = chip.bg(hsla(th.bg)).shadow(vec![BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0.18),
                offset: point(px(0.0), px(1.0)),
                blur_radius: px(3.0),
                spread_radius: px(0.0),
            }]);
        }
        if let Some(a) = &it.action {
            chip = chip.cursor_pointer().on_click(on_act(a.clone(), cx));
        }
        bar = bar.child(chip);
    }
    let wrap = div().flex();
    let wrap = if center { wrap.justify_center() } else { wrap };
    wrap.child(bar).into_any_element()
}

fn progress(p: &Paint, value: Option<f64>, label: Option<&String>, detail: Option<&String>, t: Option<&str>) -> AnyElement {
    let th = &p.view.theme;
    let s = p.size();
    let color = if t.is_none() { hsla(th.accent) } else { tone(p, t) };
    let mut col = div().flex().flex_col();
    if label.is_some() || detail.is_some() {
        col = col.child(
            div()
                .flex()
                .flex_row()
                .pb(px(6.0))
                .text_size(px(s - 1.0))
                .child(div().flex_1().min_w(px(0.0)).truncate().text_color(hsla(th.fg_dim)).child(label.cloned().unwrap_or_default()))
                .children(detail.map(|d| div().text_color(hsla(th.fg)).child(d.clone()))),
        );
    }
    let fill = match value {
        Some(v) => div().h_full().w(relative(v as f32)).bg(color),
        None => {
            let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
            let phase = (ms % 1800) as f32 / 1800.0;
            div()
                .absolute()
                .top_0()
                .h_full()
                .left(relative(phase * 1.3 - 0.3))
                .w(relative(0.3))
                .bg(color)
                .child(gpui::canvas(|_, _, _| {}, |_, _, window, _| window.request_animation_frame()))
        }
    };
    col.child(div().relative().h(px(5.0)).w_full().rounded(px(3.0)).overflow_hidden().bg(hsla(th.bg_active)).child(fill)).into_any_element()
}

fn kv(p: &Paint, items: &[(String, String, Option<String>)]) -> AnyElement {
    let th = &p.view.theme;
    let s = p.size() - 1.0;
    let mut keys = div().flex().flex_col().pr(px(16.0)).text_color(hsla(th.fg_dim));
    let mut values = div().flex_1().min_w(px(0.0)).flex().flex_col();
    for (k, v, t) in items {
        keys = keys.child(div().pb(px(6.0)).child(k.clone()));
        let color = if t.is_none() { hsla(th.fg) } else { tone(p, t.as_deref()) };
        values = values.child(div().pb(px(6.0)).text_color(color).child(v.clone()));
    }
    div().flex().flex_row().text_size(px(s)).child(keys).child(values).into_any_element()
}

fn row(p: &mut Paint, children: &[(Block, f64)], align: Option<&str>, cx: &mut Context<BlocksView>) -> AnyElement {
    let mut wrap = div().flex().flex_row().flex_wrap().gap(px(8.0)).items_center();
    wrap = match align {
        Some("center") => wrap.justify_center(),
        Some("end") => wrap.justify_end(),
        Some("between") => wrap.justify_between(),
        _ => wrap,
    };
    for (c, width) in children {
        wrap = wrap.child(match c {
            Block::Button(b) => button(p, b, cx),
            other => div().w(px(*width as f32)).flex_none().child(block(p, other, true, cx)).into_any_element(),
        });
    }
    wrap.into_any_element()
}

pub fn terminal(p: &Paint, tab_id: Option<&String>, empty: &str, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = &p.view.theme;
    let view = tab_id.and_then(|id| p.view.terminal.as_ref().and_then(|slot| slot(id, cx)));
    let frame = div().size_full().bg(hsla(th.canvas)).rounded(px(8.0)).border_1().border_color(hsla(th.border)).overflow_hidden();
    match view {
        Some(v) => frame.child(v).into_any_element(),
        None => frame
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .text_color(hsla(th.fg_faint))
            .child(empty.to_owned())
            .into_any_element(),
    }
}

pub fn expanded(p: &mut Paint, b: &Block, cx: &mut Context<BlocksView>) -> AnyElement {
    match b {
        Block::Console { id, empty, .. } => console(p, id, empty),
        Block::Terminal { tab_id, empty, .. } => terminal(p, tab_id.as_ref(), empty, cx),
        _ => div().into_any_element(),
    }
}

fn mono_note(text: String, color: Hsla) -> AnyElement {
    div().text_size(px(11.5)).font_family(mono_family()).text_color(color).child(text).into_any_element()
}

pub fn render(p: &mut Paint, b: &Block, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = p.view.theme.clone();
    let s = p.size();
    match b {
        Block::Heading { text } => div().text_size(px(s + 3.0)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(th.fg)).child(text.clone()).into_any_element(),
        Block::Text { text, style, center } => text_block(p, text, style.as_deref(), *center),
        Block::Markdown { text } => markdown(p, text),
        Block::Code { text } => div()
            .p(px(12.0))
            .bg(hsla(th.bg_active))
            .rounded(px(8.0))
            .border_1()
            .border_color(hsla(th.border))
            .font_family(mono_family())
            .text_size(px(s - 1.7))
            .line_height(relative(1.5))
            .text_color(hsla(th.fg))
            .child(text.clone())
            .into_any_element(),
        Block::Divider => div().h(px(1.0)).bg(hsla(th.border)).into_any_element(),
        Block::Section { text, count, collapsed, action, label } => section(p, text, count.as_deref(), *collapsed, action.as_ref(), *label, cx),
        Block::Button(btn) => div().flex().child(button(p, btn, cx)).into_any_element(),
        Block::Row { children, align } => row(p, children, align.as_deref(), cx),
        Block::Kv { items } => kv(p, items),
        Block::Progress { value, label, detail, tone: t } => progress(p, *value, label.as_ref(), detail.as_ref(), t.as_deref()),
        Block::Console { id, height, empty, .. } => div().h(px(*height as f32)).flex().flex_col().child(console(p, id, empty)).into_any_element(),
        Block::Header { .. } => header(p, b, cx),
        Block::Card { children, padding } => card(p, children, *padding, cx),
        Block::Tabs { items, center } => tabs(p, items, *center, cx),
        Block::Columns { children } => {
            let mut r = div().flex().flex_row().items_start().gap(px(14.0));
            for c in children {
                r = r.child(div().flex_1().min_w(px(0.0)).child(block(p, c, true, cx)));
            }
            r.into_any_element()
        }
        Block::Terminal { tab_id, height, empty, .. } => div().h(px(*height as f32)).child(terminal(p, tab_id.as_ref(), empty, cx)).into_any_element(),
        Block::Unknown { label } => mono_note(label.clone(), hsla(th.fg_faint)),
        Block::Broken { message } => mono_note(message.clone(), hsla(th.red)),
        _ => div().into_any_element(),
    }
}
