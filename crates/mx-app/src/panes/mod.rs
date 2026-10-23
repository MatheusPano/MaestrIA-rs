mod drop;
mod empty;
mod grip;
mod header;
mod terminals;

use gpui::{
    AnyElement, AnyView, App, Context, DragMoveEvent, Entity, Focusable, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, SharedString, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder, px, relative,
};
use mx_core::layout::{Axis, DropSide, PaneNode};
use mx_core::model::TabKind;

use crate::app::drag::DraggedTab;
use crate::app::host::{StoreHost, update_store};
use crate::app::theme::{Mx, RADIUS};

pub use terminals::Terminals;

pub struct PaneArea {
    host: Entity<StoreHost>,
    terminals: Terminals,
    hint: Option<(String, DropSide)>,
    resizing: Option<grip::GripDrag>,
    focused: Option<String>,
}

impl PaneArea {
    pub fn new(host: Entity<StoreHost>) -> PaneArea {
        PaneArea {
            host,
            terminals: Terminals::default(),
            hint: None,
            resizing: None,
            focused: None,
        }
    }


    fn follow_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.host.read(cx).store.focused_tab_id();
        if focused == self.focused {
            return;
        }
        self.focused = focused.clone();
        if let Some(view) = focused.as_deref().and_then(|id| self.terminals.get(id)) {
            let handle = view.focus_handle(cx);
            window.focus(&handle);
        }
    }

    fn on_drag_move(&mut self, id: &str, ev: &DragMoveEvent<DraggedTab>, cx: &mut Context<Self>) {
        let b = ev.bounds;
        let p = ev.event.position;
        let inside = b.contains(&p);
        let next = if inside {
            let side = drop::side_at(
                f32::from(p.x - b.origin.x),
                f32::from(p.y - b.origin.y),
                f32::from(b.size.width),
                f32::from(b.size.height),
            );
            Some((id.to_owned(), side))
        } else if self.hint.as_ref().is_some_and(|(h, _)| h == id) {
            None
        } else {
            return;
        };
        if next != self.hint {
            self.hint = next;
            cx.notify();
        }
    }

    fn leaf(&self, id: &str, show_focus: bool, mx: &Mx, cx: &Context<Self>) -> AnyElement {
        let store = &self.host.read(cx).store;
        let Some(tab) = store.tab(id) else {
            return div().size_full().into_any_element();
        };
        let focused = store.focused_tab_id().as_deref() == Some(id);
        let header = header::Header::of(store, mx, tab, show_focus, focused);
        let body: AnyElement = match (tab.kind, self.terminals.get(id)) {
            (TabKind::Claude | TabKind::Shell, Some(view)) => AnyView::from(view.clone())
                .cached(StyleRefinement::default().size_full())
                .into_any_element(),
            _ => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .text_color(mx.fg_faint)
                .child(if tab.hibernated { "Hibernada" } else { "" })
                .into_any_element(),
        };
        let ring = if show_focus && focused {
            mx.accent.opacity(0.65)
        } else {
            mx.border
        };
        let width = if show_focus && focused { 2. } else { 1. };
        let hint = self
            .hint
            .as_ref()
            .filter(|(h, _)| h == id && cx.has_active_drag())
            .map(|(_, side)| *side);
        let focus_host = self.host.clone();
        let drop_host = self.host.clone();
        let (move_id, focus_id, drop_id) = (id.to_owned(), id.to_owned(), id.to_owned());
        div()
            .id(SharedString::from(format!("pane:{id}")))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .rounded(px(RADIUS))
            .bg(mx.bg)
            .overflow_hidden()
            .capture_any_mouse_down(move |_, _, cx: &mut App| {
                update_store(&focus_host, cx, |s| s.focus_pane(&focus_id))
            })
            .on_drag_move(cx.listener(move |this, ev: &DragMoveEvent<DraggedTab>, _, cx| {
                this.on_drag_move(&move_id, ev, cx)
            }))
            .on_drop(cx.listener(move |this, d: &DraggedTab, _, cx| {
                let side = this
                    .hint
                    .take()
                    .filter(|(h, _)| *h == drop_id)
                    .map_or(DropSide::Center, |(_, s)| s);
                update_store(&drop_host, cx, |s| s.drop_tab(&d.id, &drop_id, side));
            }))
            .child(header.render(mx, &self.host))
            .child(div().flex_1().min_h_0().px(px(12.)).py(px(10.)).child(body))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(RADIUS))
                    .border(px(width))
                    .border_color(ring),
            )
            .when_some(hint, |d, side| d.child(drop::hint(mx, side)))
            .into_any_element()
    }

    fn node(&self, node: &PaneNode, path: Vec<usize>, show_focus: bool, mx: &Mx, cx: &Context<Self>) -> AnyElement {
        let split = match node {
            PaneNode::Leaf(id) => return self.leaf(id, show_focus, mx, cx),
            PaneNode::Split(split) => split,
        };
        let axis = split.axis;
        let mut kids: Vec<AnyElement> = Vec::new();
        for (i, (child, weight)) in split.children.iter().zip(&split.weights).enumerate() {
            if i > 0 {
                kids.push(grip::grip(
                    mx,
                    &self.host,
                    &path,
                    i - 1,
                    axis,
                    self.resizing.as_ref().is_some_and(|d| d.path == path && d.gutter == i - 1),
                ));
            }
            let mut p = path.clone();
            p.push(i);
            let el = self.node(child, p, show_focus, mx, cx);
            kids.push(
                div()
                    .flex_basis(relative(*weight as f32))
                    .flex_grow()
                    .flex_shrink()
                    .min_w_0()
                    .min_h_0()
                    .child(el)
                    .into_any_element(),
            );
        }
        let weights = split.weights.clone();
        let container = div()
            .id(SharedString::from(format!("split:{path:?}")))
            .size_full()
            .flex()
            .on_drag_move(cx.listener(move |this, ev: &DragMoveEvent<grip::GripDrag>, _, cx| {
                if ev.drag(cx).path != path {
                    return;
                }
                if this.resizing.is_none() {
                    this.resizing = Some(ev.drag(cx).clone());
                    cx.notify();
                }
                let host = this.host.clone();
                grip::on_move(&host, &path, &weights, axis, ev, cx)
            }));
        match axis {
            Axis::Row => container.flex_row(),
            Axis::Column => container.flex_col(),
        }
        .children(kids)
        .into_any_element()
    }
}

impl Render for PaneArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.read(cx);
        let typo = host.store.prefs().typography.clone();
        let shown: Vec<terminals::Shown> = host
            .store
            .order()
            .into_iter()
            .filter_map(|id| host.store.tab(&id).cloned())
            .map(|t| terminals::Shown {
                claude: t.kind == TabKind::Claude,
                cwd: t.cwd.clone(),
                type_size: (typo.size_at(t.zoom as i64) as f32, typo.line as f32),
                id: t.id,
            })
            .collect();
        let registry = host.terms.clone();
        let palette = Mx::get(cx).theme.terminal();
        self.terminals.sync(&shown, &registry, &palette, window, cx);
        self.follow_focus(window, cx);
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let tree = store.panes().cloned();
        let show_focus = store.pane_count() > 1;
        let live = store.tabs().iter().filter(|t| !t.exited && !t.embedded).count();
        let body = match tree {
            Some(tree) => self.node(&tree, Vec::new(), show_focus, &mx, cx),
            None => empty::empty_pane(&mx, &self.host, live).into_any_element(),
        };
        div()
            .size_full()
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if this.hint.take().is_some() | this.resizing.take().is_some() {
                    cx.notify();
                }
            }))
            .child(body)
    }
}
