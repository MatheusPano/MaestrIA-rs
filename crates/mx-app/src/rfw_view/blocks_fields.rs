use std::rc::Rc;

use chrono::{Datelike, Local, NaiveDate};
use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, div, point, prelude::*, px,
};
use mx_plugins::ui::blocks::{Block, select_choice};
use mx_plugins::ui::calendar::{CalendarBlock, INITIALS, selection, value_of};
use serde_json::Value;

use super::blocks::{BlocksView, Paint};
use super::icons::{named, plugin_icon};
use super::menu::{self, MenuRow};
use super::paint::ring;
use super::style::{alpha, hsla};

const CELL: f32 = 38.0;

fn frame(p: &Paint, focused: bool, label: Option<&String>, floating: bool) -> gpui::Div {
    let th = &p.view.theme;
    let border = if focused { hsla(th.accent) } else { alpha(th.fg_dim, 0.6) };
    let mut f = div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .min_h(px(40.0))
        .px(px(12.0))
        .py(px(9.0))
        .rounded(px(7.0))
        .border(px(if focused { 2.0 } else { 1.0 }))
        .border_color(border);
    if let Some(l) = label.filter(|_| floating) {
        f = f.child(
            div()
                .absolute()
                .top(px(-8.0))
                .left(px(9.0))
                .px(px(3.0))
                .bg(p.surface)
                .text_size(px(12.0))
                .text_color(if focused { hsla(th.accent) } else { hsla(th.fg_dim) })
                .child(l.clone()),
        );
    }
    f
}

fn input(p: &mut Paint, id: &str, label: Option<&String>, icon: Option<&String>, disabled: bool) -> AnyElement {
    let th = p.view.theme.clone();
    let Some(entity) = p.view.inputs.get(id).cloned() else { return div().into_any_element() };
    let focused = p.view.focused.contains(id);
    let empty = p.view.fields.text(id).is_none_or(str::is_empty) && p.view.input_texts.get(id).is_none_or(String::is_empty);
    let floating = focused || !empty;
    let mut f = frame(p, focused, label, floating).items_start();
    if disabled {
        f = f.opacity(0.5);
    }
    if let Some(i) = plugin_icon(icon.map(String::as_str)) {
        f = f.child(div().w(px(22.0)).flex_none().child(named(i, 16.0, hsla(th.fg_faint))));
    }
    if let Some(l) = label.filter(|_| !floating) {
        let s = p.size() - 0.5;
        let left = if icon.is_some() { 34.0 } else { 12.0 };
        f = f.child(div().absolute().top(px(9.0)).left(px(left)).text_size(px(s)).text_color(hsla(th.fg_dim)).child(l.clone()));
    }
    f.child(entity).into_any_element()
}

fn checkbox(p: &mut Paint, id: &str, label: &str, action: Option<&String>, disabled: bool, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = p.view.theme.clone();
    let on = p.view.fields.pick(id) == Some(&Value::Bool(true));
    let icon = if on { named("check_box", 18.0, hsla(th.accent)) } else { named("check_box_outline_blank", 18.0, hsla(th.fg_dim)) };
    let mut el = div()
        .id(p.id("check"))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .rounded(px(6.0))
        .child(icon)
        .child(div().text_size(px(p.size() - 0.5)).text_color(hsla(th.fg)).child(label.to_owned()));
    if !disabled {
        let (id, action) = (id.to_owned(), action.cloned());
        el = el.cursor_pointer().on_click(cx.listener(move |v, _, window, cx| {
            let on = v.fields.pick(&id) == Some(&Value::Bool(true));
            v.fields.set_pick(&id, Value::Bool(!on));
            if let Some(a) = &action {
                v.act(a, window, cx);
            }
            cx.notify();
        }));
    }
    div().flex().child(el).into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn select(
    p: &mut Paint,
    id: &str,
    options: &[(String, String)],
    label: Option<&String>,
    placeholder: Option<&String>,
    loading: bool,
    action: Option<&String>,
    disabled: bool,
    cx: &mut Context<BlocksView>,
) -> AnyElement {
    let th = p.view.theme.clone();
    let current = select_choice(options, p.view.fields.pick(id));
    let mut f = frame(p, false, label, true).id(p.id("select")).text_size(px(p.size() - 0.5));
    let (text, color) = match current {
        Some(i) => (options[i].1.clone(), hsla(if disabled { th.fg_dim } else { th.fg })),
        None => (placeholder.cloned().unwrap_or_default(), hsla(if loading { th.fg_dim } else { th.fg_faint })),
    };
    f = f.child(div().flex_1().min_w(px(0.0)).truncate().text_color(color).child(text));
    f = if loading {
        f.child(div().relative().size(px(14.0)).child(ring(None, 1.6, hsla(th.fg_dim), None)))
    } else {
        f.child(named("arrow_drop_down", 24.0, hsla(if disabled { th.fg_faint } else { th.fg_dim })))
    };
    if disabled || loading {
        return f.into_any_element();
    }
    let rows: Vec<MenuRow> = options
        .iter()
        .map(|(v, l)| MenuRow::Item { action: v.clone(), label: l.clone(), icon: None, color: None, disabled: false })
        .collect();
    let (id, action) = (id.to_owned(), action.cloned());
    let view = cx.weak_entity();
    f.cursor_pointer()
        .on_click(move |e, _, app| {
            let (id, action, weak) = (id.clone(), action.clone(), view.clone());
            let pick: menu::Pick = Rc::new(move |value, window, app| {
                weak.update(app, |v, cx| {
                    v.fields.set_pick(&id, Value::String(value));
                    if let Some(a) = &action {
                        v.act(a, window, cx);
                    }
                    cx.notify();
                })
                .ok();
            });
            let at = e.position() + point(px(-40.0), px(18.0));
            let rows = rows.clone();
            view.update(app, |v, cx| {
                menu::open(v, at, rows, pick);
                cx.notify();
            })
            .ok();
        })
        .into_any_element()
}

fn set_range(v: &mut BlocksView, b: &CalendarBlock, range: (NaiveDate, NaiveDate), window: &mut gpui::Window, cx: &mut Context<BlocksView>) {
    v.fields.set_pick(&b.id, value_of(b.range, range.0, range.1));
    if let Some(a) = &b.action {
        v.act(a, window, cx);
    }
    cx.notify();
}

fn day_cell(p: &mut Paint, b: &CalendarBlock, d: NaiveDate, days: u32, sel: (Option<NaiveDate>, Option<NaiveDate>), cx: &mut Context<BlocksView>) -> AnyElement {
    let th = p.view.theme.clone();
    let Some(cal) = p.view.calendars.get(&b.id) else { return div().into_any_element() };
    let off = cal.off(d);
    let today = d == Local::now().date_naive();
    let (inside, start, end) = match sel {
        (Some(f), Some(t)) => (d >= f && d <= t, d == f, d == t),
        _ => (false, false, false),
    };
    let dot = start || end;
    let mut cell = div().id(p.id("day")).relative().size(px(CELL)).flex().items_center().justify_center();
    if inside && !(start && end) {
        let wd = d.weekday().num_days_from_sunday();
        let (round_l, round_r) = ((wd == 0 || d.day() == 1) && !start, (wd == 6 || d.day() == days) && !end);
        let mut band = div().absolute().top(px(3.0)).bottom(px(3.0)).bg(alpha(th.accent, 0.18));
        band = band.left(px(if start { CELL / 2.0 } else { 0.0 })).right(px(if end { CELL / 2.0 } else { 0.0 }));
        if round_l {
            band = band.rounded_l(px(CELL / 2.0));
        }
        if round_r {
            band = band.rounded_r(px(CELL / 2.0));
        }
        cell = cell.child(band);
    }
    let color: Hsla = if off { alpha(th.fg_faint, 0.45) } else if dot { hsla(th.canvas) } else { hsla(th.fg) };
    let mut circle = div()
        .size(px(CELL - 6.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.5))
        .font_weight(if dot || today { FontWeight::BOLD } else { FontWeight::NORMAL })
        .text_color(color)
        .child(d.day().to_string());
    if dot {
        circle = circle.bg(hsla(th.accent));
    } else if today {
        circle = circle.border_1().border_color(alpha(th.accent, 0.8));
    }
    cell = cell.child(circle);
    if !off {
        let b = b.clone();
        cell = cell.cursor_pointer().on_click(cx.listener(move |v, _, window, cx| {
            let picked = v.calendars.get_mut(&b.id).and_then(|c| c.tap(d));
            if let Some(r) = picked {
                set_range(v, &b, r, window, cx);
            }
        }));
    }
    cell.into_any_element()
}

fn arrow(p: &mut Paint, icon: &'static str, enabled: bool, id: &str, next: bool, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = &p.view.theme;
    let color = if enabled { hsla(th.fg_dim) } else { alpha(th.fg_faint, 0.35) };
    let mut el = div().id(p.id("arrow")).size(px(30.0)).flex().items_center().justify_center().rounded_full().child(named(icon, 20.0, color));
    if enabled {
        let id = id.to_owned();
        el = el.cursor_pointer().on_click(cx.listener(move |v, _, _, cx| {
            if let Some(c) = v.calendars.get_mut(&id) {
                if next { c.next() } else { c.prev() }
            }
            cx.notify();
        }));
    }
    el.into_any_element()
}

fn calendar(p: &mut Paint, b: &CalendarBlock, cx: &mut Context<BlocksView>) -> AnyElement {
    let th = p.view.theme.clone();
    let Some(cal) = p.view.calendars.get(&b.id).cloned() else { return div().into_any_element() };
    let sel = selection(p.view.fields.pick(&b.id));
    let month = cal.month;
    let next_month = month.checked_add_months(chrono::Months::new(1)).unwrap_or(month);
    let days = next_month.signed_duration_since(month).num_days() as u32;
    let lead = month.weekday().num_days_from_sunday();
    let header = div()
        .w(px(CELL * 7.0))
        .flex()
        .flex_row()
        .items_center()
        .child(arrow(p, "chevron_left_rounded", cal.can_prev(), &b.id, false, cx))
        .child(div().flex_1().flex().justify_center().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).child(cal.header()))
        .child(arrow(p, "chevron_right_rounded", cal.can_next(), &b.id, true, cx));
    let mut weekdays = div().pt(px(6.0)).pb(px(2.0)).flex().flex_row();
    for i in INITIALS {
        weekdays = weekdays.child(div().w(px(CELL)).flex().justify_center().text_size(px(10.5)).font_weight(FontWeight::SEMIBOLD).text_color(hsla(th.fg_faint)).child(i));
    }
    let mut grid = div().w(px(CELL * 7.0)).flex().flex_row().flex_wrap();
    for i in 0..(lead + days).div_ceil(7) * 7 {
        grid = grid.child(if i < lead || i >= lead + days {
            div().size(px(CELL)).into_any_element()
        } else {
            day_cell(p, b, month.with_day(i - lead + 1).unwrap_or(month), days, sel, cx)
        });
    }
    let cal_col = div().flex().flex_col().items_center().child(header).child(weekdays).child(grid);
    let mut presets = div().flex().flex_col().w(px(148.0)).flex_none();
    for pr in &b.presets {
        let on = cal.anchor.is_none() && sel == (Some(pr.from), Some(pr.to));
        let hover = hsla(th.bg_hover);
        let (b2, pr2) = (b.clone(), pr.clone());
        presets = presets.child(
            div()
                .id(p.id("preset"))
                .mb(px(3.0))
                .px(px(10.0))
                .py(px(7.0))
                .rounded(px(7.0))
                .text_size(px(12.5))
                .cursor_pointer()
                .text_color(hsla(if on { th.accent } else { th.fg }))
                .font_weight(if on { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                .when(on, |d| d.bg(alpha(th.accent, 0.18)))
                .when(!on, move |d| d.hover(move |s| s.bg(hover)))
                .child(pr.label.clone())
                .on_click(cx.listener(move |v, _, window, cx| {
                    let r = v.calendars.get_mut(&b2.id).map(|c| c.preset(&pr2));
                    if let Some(r) = r {
                        set_range(v, &b2, r, window, cx);
                    }
                })),
        );
    }
    let body = if b.presets.is_empty() {
        cal_col.into_any_element()
    } else {
        div()
            .flex()
            .flex_row()
            .child(presets)
            .child(div().w(px(1.0)).mx(px(14.0)).bg(hsla(th.border)))
            .child(cal_col)
            .into_any_element()
    };
    let summary = cal.summary(sel.0, sel.1);
    let card = div()
        .p(px(14.0))
        .bg(hsla(th.bg_sidebar))
        .rounded(px(12.0))
        .border_1()
        .border_color(hsla(th.border))
        .flex()
        .flex_col()
        .child(body)
        .child(div().my(px(10.0)).h(px(1.0)).bg(hsla(th.border)))
        .child(
            div()
                .flex()
                .justify_center()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(hsla(if cal.anchor.is_some() { th.fg_dim } else { th.fg }))
                .child(summary),
        );
    div().flex().justify_center().child(card).into_any_element()
}

pub fn field(p: &mut Paint, b: &Block, cx: &mut Context<BlocksView>) -> AnyElement {
    match b {
        Block::Input { id, label, icon, disabled, .. } => input(p, id, label.as_ref(), icon.as_ref(), *disabled),
        Block::Checkbox { id, label, action, disabled } => checkbox(p, id, label, action.as_ref(), *disabled, cx),
        Block::Select { id, options, label, placeholder, loading, action, disabled } => {
            select(p, id, options, label.as_ref(), placeholder.as_ref(), *loading, action.as_ref(), *disabled, cx)
        }
        Block::Calendar(c) => calendar(p, c, cx),
        _ => div().into_any_element(),
    }
}
