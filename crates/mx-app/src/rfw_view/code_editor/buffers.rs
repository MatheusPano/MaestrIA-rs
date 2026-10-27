use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{App, Entity, EntityId, Global, WeakEntity};

use crate::app::host::StoreHost;
use crate::rfw_view::code_pane::CodePane;

pub type Alive = Rc<dyn Fn(&str, &App) -> bool>;

#[derive(Default)]
struct CodeBuffers {
    owners: HashMap<String, HashMap<String, Entity<CodePane>>>,
    views: HashMap<EntityId, String>,
    alive: Option<Alive>,
}

impl Global for CodeBuffers {}

const LOOSE: &str = "rfw:";

pub fn own_code_buffers(view: EntityId, owner: &str, host: &Entity<StoreHost>, cx: &mut App) {
    let host = host.downgrade();
    own_code_buffers_with(view, owner, Rc::new(move |o, cx| kept_by_store(&host, o, cx)), cx);
}

pub fn own_code_buffers_with(view: EntityId, owner: &str, alive: Alive, cx: &mut App) {
    let g = cx.default_global::<CodeBuffers>();
    g.views.insert(view, owner.to_owned());
    g.alive = Some(alive);
    prune(cx);
}

fn kept_by_store(host: &WeakEntity<StoreHost>, owner: &str, cx: &App) -> bool {
    let Some(host) = host.upgrade() else { return false };
    let store = &host.read(cx).store;
    if let Some(plugin) = owner.strip_prefix("sidebar:") {
        return store.sidebar_view_of(plugin).is_some();
    }
    store.plugin_view(owner).is_some() || store.plugin_ui().floats.shown().iter().any(|f| f.tab_id == owner)
}

pub fn owner_of(view: EntityId, cx: &mut App) -> String {
    let g = cx.default_global::<CodeBuffers>();
    g.views.entry(view).or_insert_with(|| format!("{LOOSE}{view}")).clone()
}

pub fn pane(owner: &str, doc: &str, cx: &App) -> Option<Entity<CodePane>> {
    cx.try_global::<CodeBuffers>()?.owners.get(owner)?.get(doc).cloned()
}

pub fn keep(owner: &str, doc: &str, pane: Entity<CodePane>, cx: &mut App) {
    let g = cx.default_global::<CodeBuffers>();
    g.owners.entry(owner.to_owned()).or_default().insert(doc.to_owned(), pane);
}

pub fn released(view: EntityId, cx: &mut App) {
    if let Some(g) = cx.try_global::<CodeBuffers>()
        && g.views.contains_key(&view)
    {
        cx.global_mut::<CodeBuffers>().views.remove(&view);
        prune(cx);
    }
}

fn prune(cx: &mut App) {
    let Some(g) = cx.try_global::<CodeBuffers>() else { return };
    let bound: HashSet<&String> = g.views.values().collect();
    let dead: Vec<String> = g
        .owners
        .keys()
        .filter(|o| !bound.contains(o))
        .filter(|o| o.starts_with(LOOSE) || !g.alive.as_ref().is_some_and(|alive| alive(o, cx)))
        .cloned()
        .collect();
    if dead.is_empty() {
        return;
    }
    let g = cx.global_mut::<CodeBuffers>();
    for o in dead {
        g.owners.remove(&o);
    }
}

#[cfg(test)]
pub fn owners(cx: &App) -> Vec<String> {
    let mut out: Vec<String> = cx.try_global::<CodeBuffers>().map(|g| g.owners.keys().cloned().collect()).unwrap_or_default();
    out.sort();
    out
}
