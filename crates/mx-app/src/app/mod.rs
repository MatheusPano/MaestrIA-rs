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
mod perf_tour;
pub mod ports;
pub mod restart;
pub mod root;
pub mod settings;
pub mod suggestions;
pub mod theme;
pub mod tooltip;
pub mod ui;
pub mod update_confirm;
pub mod window_frame;

#[cfg(test)]
mod mouse_tests;

use std::borrow::Cow;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use mx_store::StoreOptions;

use host::StoreHost;
use root::Root;

const QUIT_GRACE: std::time::Duration = std::time::Duration::from_secs(3);

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
        quitting.update(cx, |h, _| {
            if h.terms.ptyd().is_some() && !h.store.prefs().end_sessions_on_quit {
                h.store.keep_sessions_on_quit();
                if let Some(away) = h.store.hibernatable_away()
                    && let Err(err) = h.terms.hibernate_when_idle(away.claudes, away.shells, away.after)
                {
                    log::warn!("ptyd hibernation setup failed: {err}");
                }
            }
            let rt = h.rt();
            let done = h.store.shutdown();
            rt.block_on(async move {
                let _ = tokio::time::timeout(QUIT_GRACE, done).await;
            });
            if h.stop_ptyd_on_quit
                && let Some(d) = h.terms.ptyd()
                && let Err(err) = d.shutdown()
            {
                log::warn!("could not stop ptyd: {err}");
            }
            h.store.on_quit();
        });
        std::future::ready(())
    })
    .detach();
    let bounds = Bounds::centered(None, size(px(1280.), px(720.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Maximized(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("MaestrIA".into()),
            ..Default::default()
        }),
        app_id: Some(app_id()),
        ..Default::default()
    };
    let stale = host.read(cx).stale_ptyd;
    let window = cx.open_window(options, |window, cx| cx.new(|cx| Root::new(host, window, cx)))?;
    if perf_tour::requested() {
        perf_tour::start(window.into(), cx);
    }
    if let Some(sessions) = stale {
        restart::confirm(restart::Kind::StaleDaemon { sessions }, cx);
    }
    Ok(())
}

pub fn app_id() -> String {
    std::env::var("MAESTRIA_APP_ID")
        .ok()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| "br.com.marrow.maestria".into())
}
