mod bodies;
mod cards_model;
mod changes;
mod drop;
mod empty;
mod grip;
mod header;
mod header_cache;
mod plugin_tiles;
mod session_cards;
mod terminals;
mod view_header;

use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, AnyView, App, AppContext, Bounds, Context, DragMoveEvent, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, Modifiers, ModifiersChangedEvent, MouseButton, MouseMoveEvent, ParentElement, Pixels,
    Point, Render, SharedString, StyleRefinement, Styled, Subscription, Window, div, prelude::FluentBuilder, px, relative,
};
use mx_store::{Areas, PaneDrop};
use mx_core::layout::{self, Axis, DropSide, PaneNode};
use mx_core::model::TabKind;

use crate::app::drag::DraggedTab;
use crate::app::host::{StoreChanged, StoreHost, update_store};
use crate::app::theme::{GAP, Mx, RADIUS};
use crate::chrome::style::{rest_border, seat_shadow};
use crate::plugin_ui::PluginLayer;
use crate::plugin_ui::surface::SurfaceEnv;

pub use terminals::Terminals;

pub(crate) fn focus_unless_inside(handle: &FocusHandle, window: &mut Window, cx: &App) {
    if !handle.contains_focused(window, cx) {
        window.focus(handle);
    }
}

pub struct PaneArea {
    host: Entity<StoreHost>,
    terminals: Terminals,
    hint: Option<(String, DropSide)>,
    edge: Option<DropSide>,
    area: Option<Bounds<Pixels>>,
    mods: Modifiers,
    lifted: Option<String>,
    resizing: Option<grip::GripDrag>,
    cornering: Option<grip::CornerDrag>,
    focused: Option<String>,
    focused_handle: Option<FocusHandle>,
    covered: bool,
    plugin_focus: HashMap<String, FocusHandle>,
    bodies: bodies::Bodies,
    layer: Option<Entity<PluginLayer>>,
    watch: Option<Subscription>,
    activation: Option<Subscription>,
    headers: HashMap<String, Entity<header_cache::PaneHeader>>,
    changes_open: HashSet<String>,
    changes: HashMap<String, Entity<changes::ChangesView>>,
}

impl PaneArea {
    pub fn new(host: Entity<StoreHost>) -> PaneArea {
        PaneArea {
            host,
            terminals: Terminals::default(),
            hint: None,
            edge: None,
            area: None,
            mods: Modifiers::default(),
            lifted: None,
            resizing: None,
            cornering: None,
            focused: None,
            focused_handle: None,
            covered: false,
            plugin_focus: HashMap::new(),
            bodies: bodies::Bodies::default(),
            layer: None,
            watch: None,
            activation: None,
            headers: HashMap::new(),
            changes_open: HashSet::new(),
            changes: HashMap::new(),
        }
    }

    pub fn toggle_changes(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.changes_open.remove(id) {
            self.changes_open.insert(id.to_owned());
        }
        cx.notify();
    }

    pub fn close_changes(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.changes_open.remove(id) {
            cx.notify();
        }
    }

    fn sync_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store = &self.host.read(cx).store;
        self.changes_open.retain(|id| store.is_open(id));
        let shown: Vec<(String, changes::Shown)> = self
            .changes_open
            .iter()
            .map(|id| (id.clone(), changes::Shown::of(store, id)))
            .filter(|(_, s)| !s.files.is_empty())
            .collect();
        let mut keep = HashMap::new();
        for (id, s) in shown {
            let (host, area) = (self.host.clone(), cx.weak_entity());
            let view = self.changes.remove(&id).unwrap_or_else(|| cx.new(|_| changes::ChangesView::new(host, area, &id)));
            if view.update(cx, |v, _| v.set(s)) {
                window.invalidate_cached_view(view.entity_id());
            }
            keep.insert(id, view);
        }
        self.changes = keep;
    }

    pub fn terminal_views(&self) -> crate::plugin_ui::surface::TermViews {
        self.terminals.shared()
    }

    pub fn set_palette(&mut self, palette: &mx_theme::TerminalPalette, cx: &mut Context<Self>) {
        for view in self.terminals.all() {
            view.update(cx, |v, cx| v.set_palette(palette, cx));
        }
        for view in self.changes.values() {
            view.update(cx, |_, cx| cx.notify());
        }
    }

    fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.watch.is_some() {
            return;
        }
        self.activation = Some(cx.observe_window_activation(window, |_, _, cx| cx.notify()));
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
            focus_unless_inside(&handle, window, cx);
        }
    }

    fn track_mods(&mut self, mods: Modifiers, cx: &mut Context<Self>) {
        if self.mods != mods {
            self.mods = mods;
            cx.notify();
        }
    }

    fn in_edge(&self, p: Point<Pixels>) -> bool {
        self.area.is_some_and(|a| {
            let at = p - a.origin;
            drop::edge_at(f32::from(at.x), f32::from(at.y), f32::from(a.size.width), f32::from(a.size.height)).is_some()
        })
    }

    fn on_area_drag_move(&mut self, ev: &DragMoveEvent<DraggedTab>, cx: &mut Context<Self>) {
        self.track_mods(ev.event.modifiers, cx);
        let b = ev.bounds;
        if self.area != Some(b) {
            self.area = Some(b);
            cx.notify();
        }
        let store = &self.host.read(cx).store;
        let dragged = &ev.drag(cx).id;
        let movable = store.panes().is_some() && !(store.is_open(dragged) && store.pane_count() < 2);
        let at = ev.event.position - b.origin;
        let edge = movable
            .then(|| drop::edge_at(f32::from(at.x), f32::from(at.y), f32::from(b.size.width), f32::from(b.size.height)))
            .flatten();
        if edge != self.edge {
            self.edge = edge;
            if edge.is_some() {
                self.hint = None;
            }
            cx.notify();
        }
    }

    fn on_drag_move(&mut self, id: &str, ev: &DragMoveEvent<DraggedTab>, cx: &mut Context<Self>) {
        self.track_mods(ev.event.modifiers, cx);
        let dragged = ev.drag(cx).id.clone();
        if self.lifted.as_deref() != Some(dragged.as_str()) {
            let lifted = self.host.read(cx).store.is_open(&dragged).then_some(dragged.clone());
            if lifted != self.lifted {
                self.lifted = lifted;
                cx.notify();
            }
        }
        let b = ev.bounds;
        let p = ev.event.position;
        let inside = b.contains(&p) && !self.in_edge(p);
        let next = if inside && dragged == id {
            None
        } else if inside {
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
        let lifted = cx.has_active_drag() && self.lifted.as_deref() == Some(id);
        let settings: view_header::Opener = std::rc::Rc::new(|plugin, window, cx| crate::plugin_ui::open_settings(&plugin, window, cx));
        let top: Option<AnyElement> = match kind {
            TabKind::Plugin => view_header::ViewHeader::of(store, id, show_focus, focused)
                .map(|h| h.lifted(lifted).render(mx, &self.host, Some(settings), 1000.).into_any_element()),
            TabKind::Reader => None,
            _ => self.cached_header(id).or_else(|| {
                let open = self.changes_open.contains(id);
                let header = header::Header::of(store, mx, tab, show_focus, focused).lifted(lifted).files_open(open);
                Some(header.render(mx, &self.host, &cx.weak_entity()).into_any_element())
            }),
        };
        let body: AnyElement = match kind {
            TabKind::Claude | TabKind::Shell => match self.terminals.get(id) {
                Some(view) => AnyView::from(view).cached(StyleRefinement::default().size_full()).into_any_element(),
                None => bodies::placeholder(if hibernated { "Hibernada" } else { "" }, mx.fg_faint),
            },
            TabKind::Reader => self.bodies.reader(id).unwrap_or_else(|| bodies::placeholder("", mx.fg_faint)),
            TabKind::Plugin => {
                let body = self
                    .bodies
                    .surface(id)
                    .unwrap_or_else(|| bodies::placeholder(mx_plugins::ui::view::MISSING_PLUGIN, mx.fg_faint));
                match self.plugin_focus.get(id) {
                    Some(h) => div().size_full().track_focus(h).child(body).into_any_element(),
                    None => body,
                }
            }
            TabKind::Setup => self.bodies.setup(id).unwrap_or_else(|| bodies::placeholder("Configuração do projeto", mx.fg_faint)),
        };
        let ring = if show_focus && focused {
            mx.accent.opacity(0.65)
        } else {
            rest_border(&mx.theme)
        };
        let side = self.changes.get(id).map(|view| {
            div()
                .flex_none()
                .h_full()
                .w(relative(changes::SHARE))
                .min_w(px(changes::MIN_WIDTH))
                .max_w(px(changes::MAX_WIDTH))
                .child(AnyView::from(view.clone()).cached(StyleRefinement::default().size_full()))
        });
        let width = if show_focus && focused { 2. } else { 1. };
        let hint = self
            .pane_drop()
            .filter(|(h, _, d)| *h == id && !matches!(d, PaneDrop::Edge(_)) && cx.has_active_drag())
            .map(|(_, side, d)| (side, drop::label(d)));
        let focus_host = self.host.clone();
        let drop_host = self.host.clone();
        let (move_id, focus_id, drop_id, hover_id) = (id.to_owned(), id.to_owned(), id.to_owned(), id.to_owned());
        div()
            .id(SharedString::from(format!("pane:{id}")))
            .debug_selector(|| format!("pane:{id}"))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .rounded(px(RADIUS))
            .bg(mx.bg)
            .shadow(seat_shadow(&mx.theme))
            .overflow_hidden()
            .capture_any_mouse_down(move |_, _, cx: &mut App| {
                update_store(&focus_host, cx, |s| s.focus_pane(&focus_id))
            })
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, window, cx| {
                this.follow_mouse(&hover_id, ev, window, cx)
            }))
            .on_drag_move(cx.listener(move |this, ev: &DragMoveEvent<DraggedTab>, _, cx| {
                this.on_drag_move(&move_id, ev, cx)
            }))
            .on_drop(cx.listener(move |this, d: &DraggedTab, window, cx| {
                this.lifted = None;
                this.edge = None;
                let side = this
                    .hint
                    .take()
                    .filter(|(h, _)| *h == drop_id)
                    .map_or(DropSide::Center, |(_, s)| s);
                let mods = window.modifiers();
                let shown = this.host.read(cx).store.is_open(&d.id);
                let intent = drop::intent(side, mods.alt, mods.shift, shown);
                update_store(&drop_host, cx, |s| s.drop_tab(&d.id, &drop_id, intent));
            }))
            .children(top)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(div().flex_1().min_w_0().h_full().child(body))
                    .children(side),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(RADIUS))
                    .border(px(width))
                    .border_color(ring),
            )
            .when_some(hint, |d, (side, label)| d.child(drop::overlay(mx, side, 0.5, label)))
            .into_any_element()
    }

    fn follow_mouse(&mut self, id: &str, ev: &MouseMoveEvent, window: &Window, cx: &mut Context<Self>) {
        let store = &self.host.read(cx).store;
        if !store.prefs().focus_follows_mouse
            || store.focused_pane_id() == Some(id)
            || store.zoomed().is_some()
            || ev.pressed_button.is_some()
            || cx.has_active_drag()
            || self.resizing.is_some()
            || self.cornering.is_some()
            || !window.is_window_active()
            || self.overlay_open(cx)
        {
            return;
        }
        update_store(&self.host, cx, |s| s.focus_pane(id));
    }

    fn pane_drop(&self) -> Option<(&str, DropSide, PaneDrop)> {
        let (id, side) = self.hint.as_ref()?;
        Some((id.as_str(), *side, drop::intent(*side, self.mods.alt, self.mods.shift, self.lifted.is_some())))
    }

    fn leave_zoom(&mut self, cx: &mut Context<Self>) {
        if self.host.read(cx).store.zoomed().is_none() || self.overlay_open(cx) {
            return;
        }
        update_store(&self.host, cx, |s| {
            s.unzoom();
        });
        cx.stop_propagation();
    }

    fn edge_layer(&self, tree: &PaneNode, mx: &Mx, cx: &Context<Self>) -> AnyElement {
        let shown = self.edge.or_else(|| match self.pane_drop() {
            Some((_, _, PaneDrop::Edge(side))) => Some(side),
            _ => None,
        });
        let strips = [DropSide::Left, DropSide::Right, DropSide::Top, DropSide::Bottom].map(|side| {
            let host = self.host.clone();
            let strip = div()
                .id(SharedString::from(format!("pane-edge:{side:?}")))
                .absolute()
                .on_drop(cx.listener(move |this, d: &DraggedTab, _, cx| {
                    this.lifted = None;
                    this.hint = None;
                    this.edge = None;
                    update_store(&host, cx, |s| s.drop_at_edge(&d.id, side));
                }));
            let edge = px(drop::EDGE);
            match side {
                DropSide::Left => strip.left_0().top_0().h_full().w(edge),
                DropSide::Right => strip.right_0().top_0().h_full().w(edge),
                DropSide::Top => strip.left_0().top_0().w_full().h(edge),
                _ => strip.left_0().bottom_0().w_full().h(edge),
            }
        });
        div()
            .absolute()
            .inset_0()
            .when_some(shown, |d, side| {
                d.child(drop::overlay(mx, side, drop::edge_share(tree, side), drop::label(PaneDrop::Edge(side))))
            })
            .children(strips)
            .into_any_element()
    }

    fn corner_layer(&self, tree: &PaneNode, mx: &Mx, cx: &Context<Self>) -> Option<AnyElement> {
        let (w, h) = self.host.read(cx).store.pane_area()?;
        let found = layout::junctions(tree, w, h, f64::from(GAP));
        if found.is_empty() {
            return None;
        }
        let corners = found.iter().map(|j| {
            let dragging = self.cornering.as_ref().is_some_and(|d| d.rows == j.rows && d.columns == j.columns);
            grip::corner(mx, &self.host, j, dragging)
        });
        Some(div().absolute().inset_0().children(corners).into_any_element())
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
                grip::on_move(&host, &path, axis, ev, cx)
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
        self.attach(window, cx);
        let host = self.host.read(cx);
        let typo = host.store.prefs().typography.clone();
        let mut ids = host.store.order();
        ids.extend(host.store.tabs().iter().filter(|t| t.embedded).map(|t| t.id.clone()));
        let shown: Vec<terminals::Shown> = ids
            .into_iter()
            .filter_map(|id| host.store.tab(&id))
            .filter(|t| matches!(t.kind, TabKind::Claude | TabKind::Shell))
            .map(|t| terminals::Shown {
                claude: t.kind == TabKind::Claude,
                cwd: t.cwd.clone(),
                type_size: (typo.size_at(t.zoom as i64) as f32, typo.line as f32),
                id: t.id.clone(),
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
        self.terminals.repaint_focus_changes(window, cx);
        let store = &self.host.read(cx).store;
        let gone: Vec<String> = self.plugin_focus.keys().filter(|id| !store.has_tab(id)).cloned().collect();
        for id in gone {
            self.plugin_focus.remove(&id);
        }
        let mx = Mx::get(cx).clone();
        let store = &self.host.read(cx).store;
        let tree = store.panes().cloned();
        let screen = store.screen().map(str::to_owned);
        let zoomed = store.zoomed().map(str::to_owned);
        let show_focus = zoomed.is_none() && store.pane_count() > 1 && window.is_window_active();
        let ids = tree.as_ref().map(PaneNode::order).unwrap_or_default();
        self.sync_changes(window, cx);
        self.sync_headers(&ids, show_focus, &mx, window, cx);
        let edges = tree.as_ref().filter(|_| cx.has_active_drag()).map(|tree| self.edge_layer(tree, &mx, cx));
        let corners = tree.as_ref().filter(|_| zoomed.is_none()).and_then(|tree| self.corner_layer(tree, &mx, cx));
        let body = match (tree, screen) {
            (Some(tree), _) => match zoomed.as_deref() {
                Some(z) => self.leaf(z, false, &mx, cx),
                None => self.node(&tree, Vec::new(), show_focus, &mx, cx),
            },
            (None, Some(plugin)) => empty::empty_screen(&mx, &self.host, &plugin, cx).into_any_element(),
            (None, None) => empty::empty_pane(&self.host).into_any_element(),
        };
        div()
            .relative()
            .size_full()
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| {
                if this.hint.take().is_some()
                    | this.edge.take().is_some()
                    | this.resizing.take().is_some()
                    | this.cornering.take().is_some()
                    | this.lifted.take().is_some()
                {
                    cx.notify();
                }
            }))
            .on_drag_move(cx.listener(|this, ev: &DragMoveEvent<DraggedTab>, _, cx| this.on_area_drag_move(ev, cx)))
            .on_drag_move(cx.listener(|this, ev: &DragMoveEvent<grip::CornerDrag>, _, cx| {
                if this.cornering.is_none() {
                    this.cornering = Some(ev.drag(cx).clone());
                    cx.notify();
                }
                let host = this.host.clone();
                grip::on_corner_move(&host, ev, cx)
            }))
            .on_modifiers_changed(cx.listener(|this, ev: &ModifiersChangedEvent, _, cx| {
                if cx.has_active_drag() {
                    this.track_mods(ev.modifiers, cx);
                }
            }))
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                let m = ev.keystroke.modifiers;
                if ev.keystroke.key == "escape" && m.shift && !(m.control || m.alt || m.platform) {
                    this.leave_zoom(cx);
                }
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" && !ev.keystroke.modifiers.modified() {
                    this.leave_zoom(cx);
                }
            }))
            .child(body)
            .children(corners)
            .children(edges)
            .children(self.layer.clone())
    }
}
