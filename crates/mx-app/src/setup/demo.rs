use std::rc::Rc;

use gpui::{App, AppContext, Bounds, Context, Focusable, Window, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use mx_claude::setup::{MxSetup, SetupSection};
use mx_theme::Themes;

use super::{SetupActions, SetupView};
use crate::chrome::style::set_theme;

pub const ENV_ROOT: &str = "MAESTRIA_DEMO_SETUP";
pub const ENV_SELECTED: &str = "MAESTRIA_DEMO_SETUP_SELECTED";
pub const ENV_THEME: &str = "MAESTRIA_DEMO_THEME";
pub const ENV_SHOW: &str = "MAESTRIA_DEMO_SETUP_SHOW";

pub fn requested() -> Option<String> {
    std::env::var(ENV_ROOT).ok().filter(|v| !v.is_empty())
}

pub fn open(root: String, cx: &mut App) {
    crate::chrome::register_fonts(cx);
    let theme = Themes::new().by_id(std::env::var(ENV_THEME).ok().as_deref());
    set_theme(theme.clone(), cx);
    let setup = MxSetup {
        root,
        selected: std::env::var(ENV_SELECTED).ok().filter(|v| !v.is_empty()),
    };
    let actions = SetupActions {
        banner: Some(Rc::new(|text, sticky, _, _| log::info!("setup banner (sticky={sticky}): {text}"))),
        open_in_editor: Some(Rc::new(|path, _, _| log::info!("setup editor: {path}"))),
        reveal: Some(Rc::new(|path, _, _| log::info!("setup reveal: {path}"))),
        close: Some(Rc::new(|_, cx| cx.quit())),
        toggle_pin: None,
        changed: Some(Rc::new(|setup, _, _| log::info!("setup changed: {}", setup.to_json()))),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1100.), px(720.)), cx))),
        titlebar: Some(TitlebarOptions {
            title: Some("Maestria - setup".into()),
            ..Default::default()
        }),
        app_id: Some("maestria".into()),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| SetupView::new(setup, theme, actions, window, cx));
        window.focus(&view.focus_handle(cx));
        view.update(cx, |v, cx| show(v, window, cx));
        view
    });
    if let Err(err) = opened {
        log::error!("failed to open setup demo: {err:#}");
        cx.quit();
    }
}

fn show(view: &mut SetupView, window: &mut Window, cx: &mut Context<SetupView>) {
    let wanted = std::env::var(ENV_SHOW).unwrap_or_default();
    let section = |name: &str| SetupSection::ALL.into_iter().find(|s| format!("{s:?}").eq_ignore_ascii_case(name));
    match wanted.split_once(':') {
        Some(("help", name)) => {
            if let Some(s) = section(name) {
                view.explain(s, window, cx);
            }
        }
        Some(("add", name)) => {
            if let Some(s) = section(name) {
                view.add(s, window, cx);
            }
        }
        Some(("type", text)) => {
            view.editor().update(cx, |e, cx| e.insert(text, cx));
            view.editor().read(cx).focus(window);
        }
        _ if wanted == "delete" => view.delete(cx),
        _ if wanted == "focus" => view.editor().read(cx).focus(window),
        _ => {}
    }
}
