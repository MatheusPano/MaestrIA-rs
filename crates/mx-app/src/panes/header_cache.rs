use std::collections::HashMap;

use gpui::{
    AnyElement, AnyView, App, AppContext, Context, Entity, EntityId, IntoElement, ParentElement, Render, StyleRefinement,
    Styled, WeakEntity,
    Window, div, px,
};
use mx_core::model::TabKind;

use crate::app::host::StoreHost;
use crate::app::theme::Mx;
use crate::chrome::anim::slots;
use crate::panes::PaneArea;
use crate::panes::header::{HEIGHT, Header};

pub struct PaneHeader {
    host: Entity<StoreHost>,
    area: WeakEntity<PaneArea>,
    header: Option<Header>,
}

impl PaneHeader {
    fn set(&mut self, header: Header) -> bool {
        if self.header.as_ref() == Some(&header) {
            return false;
        }
        self.header = Some(header);
        true
    }
}

impl Render for PaneHeader {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        slots::forget(cx.entity_id(), cx);
        let mx = Mx::get(cx).clone();
        match self.header.clone() {
            Some(header) => div().size_full().flex().flex_col().child(header.render(&mx, &self.host, &self.area)).into_any_element(),
            None => div().into_any_element(),
        }
    }
}

impl PaneArea {
    pub fn header_views(&self) -> Vec<EntityId> {
        self.headers.values().map(|h| h.entity_id()).collect()
    }

    pub(super) fn cached_header(&self, id: &str) -> Option<AnyElement> {
        let view = self.headers.get(id)?.clone();
        let style = StyleRefinement::default().w_full().h(px(HEIGHT)).flex_none();
        Some(AnyView::from(view).cached(style).into_any_element())
    }

    fn header_of(&self, id: &str, show_focus: bool, mx: &Mx, cx: &App) -> Option<Header> {
        let store = &self.host.read(cx).store;
        let tab = store.tab(id).filter(|t| matches!(t.kind, TabKind::Claude | TabKind::Shell))?;
        let focused = store.focused_tab_id().as_deref() == Some(id);
        let lifted = cx.has_active_drag() && self.lifted.as_deref() == Some(id);
        let open = self.changes_open.contains(id);
        Some(Header::of(store, mx, tab, show_focus, focused).lifted(lifted).files_open(open))
    }

    pub(super) fn sync_headers(
        &mut self,
        ids: &[String],
        show_focus: bool,
        mx: &Mx,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut keep = HashMap::new();
        for id in ids {
            let Some(header) = self.header_of(id, show_focus, mx, cx) else { continue };
            let (host, area) = (self.host.clone(), cx.weak_entity());
            let view = self.headers.remove(id).unwrap_or_else(|| cx.new(|_| PaneHeader { host, area, header: None }));
            if view.update(cx, |h, _| h.set(header)) {
                window.invalidate_cached_view(view.entity_id());
            }
            keep.insert(id.clone(), view);
        }
        for (_, gone) in std::mem::replace(&mut self.headers, keep) {
            slots::forget(gone.entity_id(), cx);
        }
    }
}
