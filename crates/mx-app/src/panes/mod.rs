mod bodies;
mod drop;
mod empty;
mod grip;
mod header;
mod terminals;
mod view_header;

use std::collections::HashMap;

use gpui::{
    AnyElement, AnyView, App, AppContext, Context, DragMoveEvent, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, SharedString, StyleRefinement, Styled,
    Subscription, Window, div, prelude::FluentBuilder, px, relative,
};
use mx_store::Areas;
use mx_core::layout::{Axis, DropSide, PaneNode};
use mx_core::model::TabKind;

use crate::app::drag::DraggedTab;
use crate::app::host::{StoreChanged, StoreHost, update_store};
use crate::app::theme::{Mx, RADIUS};
use crate::plugin_ui::PluginLayer;
use crate::plugin_ui::surface::SurfaceEnv;

pub use terminals::Terminals;

pub struct PaneArea {
    host: Entity<StoreHost>,
    terminals: Terminals,
    hint: Option<(String, DropSide)>,
    resizing: Option<grip::GripDrag>,
    focused: Option<String>,
    focused_handle: Option<FocusHandle>,
    covered: bool,
    plugin_focus: HashMap<String, FocusHandle>,
    bodies: bodies::Bodies,
    layer: Option<Entity<PluginLayer>>,
    watch: Option<Subscription>,
}

impl PaneArea {
    pub fn new(host: Entity<StoreHost>) -> PaneArea {
        PaneArea {
            host,
            terminals: Terminals::default(),
            hint: None,
            resizing: None,
            focused: None,
            focused_handle: None,
            covered: false,
            plugin_focus: HashMap::new(),
            bodies: bodies::Bodies::default(),
            layer: None,
            watch: None,
        }
    }

    pub fn set_palette(&mut self, palette: &mx_theme::TerminalPalette, cx: &mut Context<Self>) {
        for view in self.terminals.all() {
            view.update(cx, |v, cx| v.set_palette(palette, cx));
        }
    }

    fn attach(&mut self, cx: &mut Context<Self>) {
        if self.watch.is_some() {
            return;
        }
        self.watch = Some(cx.subscribe(&self.host, |_, _, ev: &StoreChanged, cx| {
            if ev.0.areas.intersects(Areas::PLUGINS) {
                cx.notify();
            }
        }));
        let (host, terms) = (self.host.clone(), self.terminals.shared());
        self.layer = Some(cx.new(|cx| PluginLayer::new(host, terms, cx)));
    }


    fn focus_target(&mut self, id: &str, cx: &mut Context<Self>) -> Option<FocusHandle> {
        if let Some(view) = self.terminals.get(id) {
            return Some(view.focus_handle(cx));
        }
        if let Some(handle) = self.bodies.focus_target(id, cx) {
            return Some(handle);
        }
        let plugin = self.host.read(cx).store.tab(id).is_some_and(|t| t.kind == TabKind::Plugin);
        plugin.then(|| self.plugin_focus.entry(id.to_owned()).or_insert_with(|| cx.focus_handle()).clone())
    }

    fn overlay_open(&self, cx: &App) -> bool {
        crate::app::ui::get(cx).read(cx).top().is_some()
            || self.host.read(cx).store.modal().is_some()
            || self.layer.as_ref().is_some_and(|l| l.read(cx).covers())
    }

    fn follow_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let covered = self.overlay_open(cx);
        let uncovered = std::mem::replace(&mut self.covered, covered) && !covered;
        if covered {
            return;
        }
        let focused = self.host.read(cx).store.focused_tab_id();
        let handle = focused.clone().and_then(|id| self.focus_target(&id, cx));
        if !uncovered && focused == self.focused && handle == self.focused_handle {
            return;
        }
        self.focused = focused;
        self.focused_handle = handle.clone();
        if let Some(handle) = handle {
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
        let kind = tab.kind;
        let hibernated = tab.hibernated;
        let settings: view_header::Opener = std::rc::Rc::new(|plugin, window, cx| crate::plugin_ui::open_settings(&plugin, window, cx));
        let top: Option<AnyElement> = match kind {
            TabKind::Plugin => view_header::ViewHeader::of(store, id, show_focus, focused)
                .map(|h| h.render(mx, &self.host, Some(settings), 1000.).into_any_element()),
            TabKind::Reader => None,
            _ => Some(header::Header::of(store, mx, tab, show_focus, focused).render(mx, &self.host).into_any_element()),
        };
        let (body, padded): (AnyElement, bool) = match kind {
            TabKind::Claude | TabKind::Shell => match self.terminals.get(id) {
                Some(view) => (AnyView::from(view).cached(StyleRefinement::default().size_full()).into_any_element(), true),
                None => (bodies::placeholder(if hibernated { "Hibernada" } else { "" }, mx.fg_faint), true),
            },
            TabKind::Reader => (self.bodies.reader(id).unwrap_or_else(|| bodies::placeholder("", mx.fg_faint)), false),
            TabKind::Plugin => {
                let body = self
                    .bodies
                    .surface(id)
                    .unwrap_or_else(|| bodies::placeholder(mx_plugins::ui::view::MISSING_PLUGIN, mx.fg_faint));
                let body = match self.plugin_focus.get(id) {
                    Some(h) => div().size_full().track_focus(h).child(body).into_any_element(),
                    None => body,
                };
                (body, false)
            }
            TabKind::Setup => (self.bodies.setup(id).unwrap_or_else(|| bodies::placeholder("Configuração do projeto", mx.fg_faint)), false),
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
            .children(top)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when(padded, |d| d.px(px(12.)).py(px(10.)))
                    .child(body),
            )
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
        crate::chrome::anim::slots::forget(cx.entity_id(), cx);
        self.attach(cx);
        let host = self.host.read(cx);
        let typo = host.store.prefs().typography.clone();
        let mut ids = host.store.order();
        ids.extend(host.store.tabs().iter().filter(|t| t.embedded).map(|t| t.id.clone()));
        let shown: Vec<terminals::Shown> = ids
            .into_iter()
            .filter_map(|id| host.store.tab(&id).cloned())
            .filter(|t| matches!(t.kind, TabKind::Claude | TabKind::Shell))
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
        let env = SurfaceEnv {
            host: self.host.clone(),
            theme: Mx::get(cx).theme.clone(),
            typography: crate::plugin_ui::surface::rfw_typography(&typo),
            terms: self.terminals.shared(),
            zoom: 0.0,
        };
        self.bodies.sync(&env, window, cx);
        self.follow_focus(window, cx);
        let store = &self.host.read(cx).store;
        let gone: Vec<String> = self.plugin_focus.keys().filter(|id| !store.has_tab(id)).cloned().collect();
        for id in gone {
            self.plugin_focus.remove(&id);
        }
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let tree = store.panes().cloned();
        let screen = store.screen().map(str::to_owned);
        let show_focus = store.pane_count() > 1;
        let live = store.tabs().iter().filter(|t| !t.exited && !t.embedded).count();
        let body = match (tree, screen) {
            (Some(tree), _) => self.node(&tree, Vec::new(), show_focus, &mx, cx),
            (None, Some(plugin)) => empty::empty_screen(&mx, &self.host, &plugin, cx).into_any_element(),
            (None, None) => empty::empty_pane(&mx, &self.host, live).into_any_element(),
        };
        div()
            .size_full()
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if this.hint.take().is_some() | this.resizing.take().is_some() {
                    cx.notify();
                }
            }))
            .child(body)
            .children(self.layer.clone())
    }
}
