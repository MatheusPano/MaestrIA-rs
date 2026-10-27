use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, point, px,
};
use mx_plugins::ui::blocks::{Button, MenuEntry};
use mx_plugins::ui::icons::Tone;

use super::blocks::{BlocksView, Paint};
use super::icons::{named, plugin_icon};
use super::menu::{self, MenuRow};
use super::style::{alpha, hsla};
use super::tooltip;

pub fn tone(p: &Paint, raw: Option<&str>) -> Hsla {
    hsla(Tone::parse(raw).color(&p.view.theme))
}

pub fn on_act(action: String, cx: &mut Context<BlocksView>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    cx.listener(move |v, _: &ClickEvent, window, cx| {
        cx.stop_propagation();
        v.act(&action, window, cx);
    })
}

pub fn menu_rows(p: &Paint, entries: &[MenuEntry]) -> Vec<MenuRow> {
    entries
        .iter()
        .map(|e| match e {
            MenuEntry::Divider => MenuRow::Divider,
            MenuEntry::Item { action, label, icon, tone: t, disabled } => MenuRow::Item {
                action: action.clone(),
                label: label.clone(),
                icon: plugin_icon(icon.as_deref()),
                color: t.as_ref().map(|t| tone(p, Some(t))),
                disabled: *disabled,
            },
            MenuEntry::Submenu { label, icon, children } => MenuRow::Sub {
                label: label.clone(),
                icon: plugin_icon(icon.as_deref()),
                children: children
                    .iter()
                    .map(|(a, l, t)| (a.clone(), l.clone(), t.as_deref().filter(|t| !t.is_empty()).map(|t| tone(p, Some(t)))))
                    .collect(),
            },
        })
        .collect()
}

pub fn open_menu(rows: Vec<MenuRow>, cx: &mut Context<BlocksView>) -> impl Fn(gpui::Point<gpui::Pixels>, &mut App) + 'static {
    let view = cx.weak_entity();
    move |at, app| {
        let weak = view.clone();
        let pick: menu::Pick = Rc::new(move |action, window, app| {
            weak.update(app, |v, cx| v.act(&action, window, cx)).ok();
        });
        let rows = rows.clone();
        view.update(app, |v, cx| {
            menu::open(v, at, rows, pick);
            cx.notify();
        })
        .ok();
    }
}

fn icon_face(p: &mut Paint, icon: &'static str, color: Hsla, tip: Option<String>, enabled: bool) -> gpui::Stateful<gpui::Div> {
    let fg = p.view.theme.fg;
    let mut el = div().id(p.id("ib")).size(px(30.0)).flex_none().flex().items_center().justify_center().rounded_full();
    let color = if enabled { color } else { alpha(p.view.theme.fg_faint, 0.5) };
    el = el.child(named(icon, 17.0, color));
    if let Some(t) = tip {
        el = el.tooltip(tooltip::builder(t, &p.view.theme));
    }
    if enabled {
        el = el.cursor_pointer().hover(move |s| s.bg(alpha(fg, 0.08)));
    }
    el
}

pub fn icon_button(p: &mut Paint, icon: &'static str, color: Hsla, tip: Option<String>, action: Option<String>, cx: &mut Context<BlocksView>) -> AnyElement {
    let el = icon_face(p, icon, color, tip, action.is_some());
    match action {
        Some(a) => el.on_click(on_act(a, cx)).into_any_element(),
        None => el.into_any_element(),
    }
}

pub fn button(p: &mut Paint, b: &Button, cx: &mut Context<BlocksView>) -> AnyElement {
    let action = if b.disabled { None } else { b.action.clone() };
    let has_menu = !b.menu.is_empty() && !b.disabled;
    let icon = plugin_icon(b.icon.as_deref());
    if b.icon_only() {
        let color = if b.tone.is_none() { hsla(p.view.theme.fg_dim) } else { tone(p, b.tone.as_deref()) };
        let name = icon.unwrap_or("circle");
        let el = if has_menu {
            icon_face(p, name, color, b.shown_tooltip(), true).into_any_element()
        } else {
            icon_button(p, name, color, b.shown_tooltip(), action, cx)
        };
        return with_menu(p, el, b, cx);
    }
    let th = &p.view.theme;
    let (bg, fg, border) = match b.style.as_deref() {
        Some("primary") => (Some(hsla(th.accent)), hsla(on_accent_m3(th)), None),
        Some("danger") if icon.is_none() => (None, hsla(th.red), Some(alpha(th.red, 0.6))),
        _ => (None, hsla(th.accent), Some(alpha(th.fg_dim, 0.6))),
    };
    let enabled = action.is_some() || has_menu;
    let (bg, fg) = if enabled { (bg, fg) } else { (bg.map(|_| alpha(th.fg, 0.12)), alpha(th.fg, 0.38)) };
    let mut el = div()
        .id(p.id("btn"))
        .h(px(40.0))
        .pl(px(if icon.is_some() { 16.0 } else { 24.0 }))
        .pr(px(24.0))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .rounded(px(20.0))
        .text_size(px(14.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg)
        .children(icon.map(|i| named(i, 15.0, fg)))
        .child(b.shown_label());
    if let Some(bg) = bg {
        el = el.bg(bg);
    }
    if let Some(border) = border {
        el = el.border_1().border_color(if enabled { border } else { alpha(th.fg, 0.12) });
    }
    if enabled {
        let hover = alpha(th.accent, 0.08);
        el = el.cursor_pointer().hover(move |s| s.bg(hover));
        if let Some(a) = action.filter(|_| !has_menu) {
            el = el.on_click(on_act(a, cx));
        }
    }
    with_menu(p, el.into_any_element(), b, cx)
}

fn on_accent_m3(th: &mx_theme::Theme) -> mx_theme::Color {
    th.on_accent()
}

fn with_menu(p: &mut Paint, el: AnyElement, b: &Button, cx: &mut Context<BlocksView>) -> AnyElement {
    if b.menu.is_empty() || b.disabled {
        return el;
    }
    let open = open_menu(menu_rows(p, &b.menu), cx);
    div()
        .id(p.id("menu-btn"))
        .debug_selector(|| "block-menu".into())
        .flex_none()
        .child(el)
        .on_click(move |e, _, app| {
            app.stop_propagation();
            open(e.position() + point(px(-12.0), px(18.0)), app);
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{AppContext, Context, Entity, IntoElement, Modifiers, ParentElement, Render, Styled, TestAppContext, Window, deferred, div};
    use mx_theme::{Typography, default_theme};
    use serde_json::json;

    use super::super::ViewAction;
    use super::super::blocks::{BlocksMode, BlocksView};

    fn menu_header() -> serde_json::Map<String, serde_json::Value> {
        json!({"type": "header", "title": "TASK#1", "actions": [
            {"icon": "more", "tooltip": "mais", "menu": [{"action": "link:1", "label": "copiar o link"}]}
        ]})
        .as_object()
        .unwrap()
        .clone()
    }

    struct Overlay(Entity<BlocksView>);

    impl Render for Overlay {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let menu = self.0.update(cx, |v, cx| v.menu_overlay(cx));
            div()
                .size_full()
                .child(deferred(div().size_full().child(self.0.clone())).with_priority(2))
                .children(menu.map(|m| deferred(m).with_priority(2)))
        }
    }

    #[gpui::test]
    fn a_menu_opened_inside_a_modal_is_drawn_by_the_host_instead_of_a_nested_deferred(cx: &mut TestAppContext) {
        let slot: Rc<RefCell<Option<Entity<BlocksView>>>> = Rc::default();
        let slot2 = slot.clone();
        let (_, cx) = cx.add_window_view(move |_, cx| {
            let view = cx.new(|cx| {
                let mut v = BlocksView::new(default_theme(), &Typography::standard(), None, BlocksMode::Modal, Rc::new(|_, _, _| {}));
                v.hoist_menu();
                v.set_blocks(vec![menu_header()], cx);
                v
            });
            *slot2.borrow_mut() = Some(view.clone());
            Overlay(view)
        });
        let view = slot.borrow().clone().unwrap();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let b = cx.debug_bounds("block-menu").expect("menu button drawn");
        cx.simulate_click(b.center(), Modifiers::none());
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(view.read_with(cx, |v, _| v.menu.is_some()));
    }

    #[gpui::test]
    fn an_icon_only_header_menu_opens_on_click_and_sends_only_the_picked_action(cx: &mut TestAppContext) {
        let sent: Rc<RefCell<Vec<ViewAction>>> = Rc::default();
        let s = sent.clone();
        let slot: Rc<RefCell<Option<Entity<BlocksView>>>> = Rc::default();
        let slot2 = slot.clone();
        let (_, cx) = cx.add_window_view(move |_, cx| {
            let sink: super::super::ActionSink = Rc::new(move |a, _, _| s.borrow_mut().push(a));
            let mut v = BlocksView::new(default_theme(), &Typography::standard(), None, BlocksMode::Framed, sink);
            v.set_blocks(vec![menu_header()], cx);
            *slot2.borrow_mut() = Some(cx.entity());
            v
        });
        let view = slot.borrow().clone().unwrap();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let b = cx.debug_bounds("block-menu").expect("menu button drawn");
        cx.simulate_click(b.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(sent.borrow().is_empty(), "sent {:?}", sent.borrow());
        assert!(view.read_with(cx, |v, _| v.menu.is_some()));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let item = cx.debug_bounds("menu-item:copiar o link").expect("menu item drawn");
        cx.simulate_click(item.center(), Modifiers::none());
        cx.run_until_parked();
        let sent = sent.borrow();
        assert_eq!(sent.iter().map(|a| a.action.as_str()).collect::<Vec<_>>(), ["link:1"]);
        assert!(view.read_with(cx, |v, _| v.menu.is_none()));
    }
}
