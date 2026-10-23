use gpui::{
    AnyElement, App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use mx_plugins::rail::{RailBadge, rail_tooltip};
use mx_store::Store;

use super::{rail_button, text_badge};
use crate::app::host::{StoreHost, update_store};
use crate::app::theme::Mx;
use crate::app::tooltip::tip;
use crate::rfw_view::plugin_glyph;

#[derive(Clone)]
pub struct RailDrag {
    pub id: String,
    icon: Option<String>,
    dir: Option<std::path::PathBuf>,
}

impl Render for RailDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mx = Mx::get(cx);
        div()
            .size(px(super::WIDTH))
            .rounded(px(8.))
            .bg(mx.bg_sidebar)
            .border_1()
            .border_color(mx.accent.opacity(0.55))
            .flex()
            .items_center()
            .justify_center()
            .child(plugin_glyph(self.icon.as_deref(), self.dir.as_deref(), 18., mx.fg))
    }
}

pub struct RailItem {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub dir: Option<std::path::PathBuf>,
    pub current: bool,
    pub badge: Option<RailBadge>,
    pub open: usize,
}

pub fn items(store: &Store) -> Vec<RailItem> {
    let Some(r) = store.plugins() else { return Vec::new() };
    let shown = store.shown_plugin().filter(|_| !store.prefs().sidebar_hidden);
    store
        .rail_plugins()
        .into_iter()
        .filter_map(|id| {
            let p = r.get(&id)?;
            Some(RailItem {
                name: p.name().to_owned(),
                icon: p.manifest.as_ref().and_then(|m| m.icon.clone()),
                dir: Some(p.dir.clone()),
                current: shown.as_deref() == Some(id.as_str()),
                badge: store.rail_badge(&id),
                open: store.open_windows(&id),
                id,
            })
        })
        .collect()
}

pub fn button(mx: &Mx, host: &Entity<StoreHost>, at: usize, item: RailItem) -> AnyElement {
    let color = if item.current { mx.fg } else { mx.fg_faint };
    let (click_host, drop_host) = (host.clone(), host.clone());
    let id = item.id.clone();
    let drag = RailDrag { id: item.id.clone(), icon: item.icon.clone(), dir: item.dir.clone() };
    let badge: Option<AnyElement> = match &item.badge {
        Some(RailBadge::Busy) => Some(div().size(px(8.)).rounded(px(4.)).bg(mx.accent).border_1().border_color(mx.canvas).into_any_element()),
        Some(RailBadge::Said(text)) => Some(text_badge(mx, text.clone(), mx.accent).into_any_element()),
        Some(RailBadge::Windows(n)) => Some(text_badge(mx, n.to_string(), mx.fg_faint).into_any_element()),
        None => None,
    };
    rail_button(mx, SharedString::from(format!("rail-plugin:{}", item.id)), item.current)
        .child(plugin_glyph(item.icon.as_deref(), item.dir.as_deref(), 18., color))
        .tooltip(tip(SharedString::from(rail_tooltip(&item.name, item.open))))
        .on_click(move |_, _, cx: &mut App| {
            let id = id.clone();
            update_store(&click_host, cx, |s| s.show_sidebar_view(Some(&id)))
        })
        .on_drag(drag, |d: &RailDrag, _, _, cx| cx.new(|_| d.clone()))
        .drag_over::<RailDrag>(|s, _, _, _| s.border_color(gpui::transparent_black()).bg(gpui::transparent_black()))
        .on_drop(move |d: &RailDrag, _, cx: &mut App| {
            let moved = d.id.clone();
            update_store(&drop_host, cx, |s| s.move_rail_plugin(&moved, at))
        })
        .when_some(badge, |d, b| d.child(div().absolute().right(px(1.)).bottom(px(1.)).child(b)))
        .into_any_element()
}
