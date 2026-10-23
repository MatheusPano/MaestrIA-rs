pub mod assets;
pub mod avatar;
pub mod chats;
pub mod confirms;
pub mod dialogs;
pub mod drag;
pub mod flows;
pub mod host;
pub mod icons;
pub mod keys;
pub mod menu_actions;
pub mod menus;
pub mod notices;
pub mod panel_actions;
pub mod ports;
pub mod root;
pub mod settings;
pub mod theme;
pub mod tooltip;
pub mod ui;

use std::borrow::Cow;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use mx_store::StoreOptions;

use host::StoreHost;
use root::Root;

pub fn open_main_window(cx: &mut App) -> anyhow::Result<()> {
    if let Err(err) = cx.text_system().add_fonts(vec![Cow::Borrowed(icons::ICON_FONT)]) {
        log::error!("failed to load the shell icon font: {err}");
    }
    crate::chrome::register_fonts(cx);
    let host = StoreHost::boot(StoreOptions::from_env(), cx)?;
    crate::terminal_glue::install(&host, cx);
    ui::install(host.clone(), cx);
    let quitting = host.clone();
    cx.on_app_quit(move |cx| {
        quitting.update(cx, |h, _| drop(h.store.shutdown()));
        cx.background_executor().timer(std::time::Duration::from_millis(300))
    })
    .detach();
    let bounds = Bounds::centered(None, size(px(1280.), px(720.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("MaestrIA".into()),
            ..Default::default()
        }),
        app_id: Some("br.com.marrow.maestria".into()),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| cx.new(|cx| Root::new(host, window, cx)))?;
    Ok(())
}
