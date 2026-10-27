use std::rc::Rc;

use gpui::{AnyView, App, AppContext, Context, Entity, Global, Pixels, Point, Window};
use mx_store::{Areas, Store};

use crate::app::host::{StoreChanged, StoreHost, update_store};
use crate::app::notices;
use crate::chrome::anim::ConfettiLayer;
use crate::chrome::menu::{ContextMenu, MenuEntry};
use crate::chrome::notices::{NoticeCenter, ToastStack};
use crate::chrome::settings::SettingsDialog;
use crate::chrome::widgets::tip_gate::{self, Overlays};

#[derive(Clone)]
pub enum Layer {
    Menu(AnyView),
    Modal(AnyView),
    Settings(Entity<SettingsDialog>),
}

impl Layer {
    pub fn view(&self) -> AnyView {
        match self {
            Layer::Menu(v) | Layer::Modal(v) => v.clone(),
            Layer::Settings(d) => d.clone().into(),
        }
    }
}

pub struct Ui {
    pub host: Entity<StoreHost>,
    layers: Vec<(u64, Layer)>,
    next: u64,
    pub confetti: Entity<ConfettiLayer>,
    pub toasts: Entity<ToastStack>,
    pub center: Option<Entity<NoticeCenter>>,
}

struct UiHandle(Entity<Ui>);

impl Global for UiHandle {}

pub fn install(host: Entity<StoreHost>, cx: &mut App) -> Entity<Ui> {
    let toasts = cx.new(|_| ToastStack::new(notices::toast_handlers()));
    let confetti = cx.new(|_| ConfettiLayer::new());
    let ui = cx.new(|cx| {
        cx.subscribe(&host, |ui: &mut Ui, _, ev: &StoreChanged, cx| ui.on_store(ev, cx)).detach();
        Ui { host, layers: Vec::new(), next: 0, confetti, toasts, center: None }
    });
    cx.set_global(UiHandle(ui.clone()));
    ui.update(cx, |ui, cx| ui.sync_notices(cx));
    ui
}

pub fn get(cx: &App) -> Entity<Ui> {
    cx.global::<UiHandle>().0.clone()
}

pub fn host(cx: &App) -> Entity<StoreHost> {
    get(cx).read(cx).host.clone()
}

pub fn store(cx: &App) -> &Store {
    let ui = cx.global::<UiHandle>().0.read(cx);
    &ui.host.read(cx).store
}

pub fn act<R>(cx: &mut App, f: impl FnOnce(&mut Store) -> R) -> R {
    let host = host(cx);
    update_store(&host, cx, f)
}

impl Ui {
    fn sync_tips(&self, cx: &mut App) {
        let menu_on_top = matches!(self.layers.last(), Some((_, Layer::Menu(_))));
        tip_gate::set_overlays(Overlays { generation: self.next, count: self.layers.len(), menu_on_top }, cx);
    }

    pub fn views(&self) -> Vec<AnyView> {
        self.layers.iter().map(|(_, l)| l.view()).collect()
    }

    pub fn top(&self) -> Option<Layer> {
        self.layers.last().map(|(_, l)| l.clone())
    }

    fn settings(&self) -> Option<Entity<SettingsDialog>> {
        self.layers.iter().find_map(|(_, l)| match l {
            Layer::Settings(d) => Some(d.clone()),
            _ => None,
        })
    }

    fn on_store(&mut self, ev: &StoreChanged, cx: &mut Context<Self>) {
        let a = ev.0.areas;
        if a.intersects(Areas::NOTICES | Areas::PREFS) || !ev.0.tabs.is_empty() {
            self.sync_notices(cx);
        }
        if let Some(dialog) = self.settings()
            && (a.intersects(Areas::PREFS | Areas::LAUNCHERS | Areas::PLUGINS) || !ev.0.tabs.is_empty())
        {
            let data = crate::app::settings::data(&self.host, cx);
            dialog.update(cx, |d, cx| d.set_data(data, cx));
        }
    }

    pub fn sync_notices(&mut self, cx: &mut Context<Self>) {
        let store = &self.host.read(cx).store;
        let open = store.bell().notices_open;
        let dnd = store.prefs().do_not_disturb;
        let top = store.prefs().status_bar_top;
        let toasts = notices::toast_views(store);
        let list = open.then(|| notices::list_views(store));
        self.toasts.update(cx, |t, cx| {
            t.set_on_top(top, cx);
            t.set(toasts, cx);
        });
        match list {
            Some(list) => {
                let center = self
                    .center
                    .get_or_insert_with(|| cx.new(|_| NoticeCenter::new(notices::center_props())))
                    .clone();
                center.update(cx, |c, cx| c.set(list, dnd, cx));
            }
            None => self.center = None,
        }
        cx.notify();
    }
}

pub fn upcoming(cx: &App) -> u64 {
    get(cx).read(cx).next + 1
}

pub fn show(layer: impl FnOnce(&mut App) -> Layer, cx: &mut App) -> u64 {
    let layer = layer(cx);
    get(cx).update(cx, |ui, cx| {
        ui.next += 1;
        ui.layers.push((ui.next, layer));
        ui.sync_tips(cx);
        cx.notify();
        ui.next
    })
}

pub fn modal(view: AnyView, cx: &mut App) -> u64 {
    show(|_| Layer::Modal(view), cx)
}

pub fn close_if(generation: u64, cx: &mut App) {
    get(cx).update(cx, |ui, cx| {
        let before = ui.layers.len();
        ui.layers.retain(|(g, _)| *g != generation);
        if ui.layers.len() != before {
            ui.sync_tips(cx);
            cx.notify();
        }
    });
}

pub fn close_top(cx: &mut App) {
    get(cx).update(cx, |ui, cx| {
        if ui.layers.pop().is_some() {
            ui.sync_tips(cx);
            cx.notify();
        }
    });
}

pub fn close_settings(cx: &mut App) {
    get(cx).update(cx, |ui, cx| {
        ui.layers.retain(|(_, l)| !matches!(l, Layer::Settings(_)));
        ui.sync_tips(cx);
        cx.notify();
    });
}

pub fn menu<T: Clone + 'static>(
    entries: Vec<MenuEntry<T>>,
    at: Point<Pixels>,
    on_pick: impl Fn(T, &mut Window, &mut App) + 'static,
    cx: &mut App,
) -> Option<Entity<ContextMenu<T>>> {
    if entries.is_empty() {
        return None;
    }
    let generation = upcoming(cx);
    let pick = Rc::new(on_pick);
    let view = cx.new(|_| {
        ContextMenu::new(
            entries,
            at,
            move |v, w, cx| pick(v, w, cx),
            move |_, cx| close_if(generation, cx),
        )
    });
    show(|_| Layer::Menu(view.clone().into()), cx);
    Some(view)
}

pub fn fire_confetti(cx: &mut App) {
    let layer = get(cx).read(cx).confetti.clone();
    layer.update(cx, |c, cx| c.fire(cx));
}

pub fn puff(at: Point<Pixels>, window: &Window, cx: &mut App) {
    let layer = get(cx).read(cx).confetti.clone();
    layer.update(cx, |c, cx| c.puff(at, window, cx));
}

pub fn escape(window: &mut Window, cx: &mut App) -> bool {
    let Some(layer) = get(cx).read(cx).top() else { return false };
    let closes = match &layer {
        Layer::Menu(_) => true,
        Layer::Modal(_) => {
            let Some(dismiss) = crate::chrome::dialog::frame::escape_target(cx) else { return false };
            dismiss(window, cx);
            return true;
        }
        Layer::Settings(d) => d.read(cx).recording().is_none(),
    };
    if closes {
        close_top(cx);
    }
    closes
}
