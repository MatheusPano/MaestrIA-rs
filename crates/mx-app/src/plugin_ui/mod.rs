mod floats;
mod form;
mod install;
mod layer;
mod log;
mod modal;
mod pick;
mod settings;
mod sidebar;
pub mod surface;
mod widgets;

use gpui::{AnyView, App, Entity, Global, WeakEntity, Window};

pub use layer::PluginLayer;

struct LayerHandle(WeakEntity<PluginLayer>, Entity<sidebar::PluginSidebar>);

impl Global for LayerHandle {}

pub fn sidebar_view(cx: &App) -> Option<AnyView> {
    let store = &cx.try_global::<LayerHandle>()?.1;
    Some(store.clone().into())
}

pub fn open_settings(plugin: &str, window: &mut Window, cx: &mut App) {
    let Some(layer) = cx.try_global::<LayerHandle>().and_then(|h| h.0.upgrade()) else { return };
    let plugin = plugin.to_owned();
    layer.update(cx, |l, cx| l.open_settings(&plugin, window, cx));
}

pub fn open_install(window: &mut Window, cx: &mut App) {
    let Some(layer) = cx.try_global::<LayerHandle>().and_then(|h| h.0.upgrade()) else { return };
    layer.update(cx, |l, cx| l.open_install(window, cx));
}

pub fn confirm_uninstall(plugin: &str, cx: &mut App) {
    let Some(info) = crate::app::ui::store(cx).plugins().and_then(|r| r.get(plugin)) else { return };
    let id = plugin.to_owned();
    crate::app::confirms::ask(install::uninstall_confirm(&info), cx, move |_, _, cx| {
        crate::app::ui::act(cx, |s| s.uninstall_plugin(&id));
    });
}

pub fn conflicts(store: &mx_store::Store, plugin: &str) -> usize {
    let Some(r) = store.plugins() else { return 0 };
    let linux = crate::keymap::LINUX;
    let map = crate::keymap::keymap_of(store);
    r.commands()
        .iter()
        .filter(|c| c.plugin_id == plugin)
        .filter_map(|c| c.key)
        .filter(|k| map.owner(&k.for_host(linux)).is_some())
        .count()
}

pub fn open_log(plugin: &str, window: &mut Window, cx: &mut App) {
    let Some(layer) = cx.try_global::<LayerHandle>().and_then(|h| h.0.upgrade()) else { return };
    let plugin = plugin.to_owned();
    layer.update(cx, |l, cx| l.open_log(&plugin, window, cx));
}
