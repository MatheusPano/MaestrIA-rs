#![forbid(unsafe_code)]

mod app;
mod chrome;
mod panes;
mod rail;
mod reader;
mod rfw_view;
mod sidebar;
mod status_bar;
mod terminal_element;
mod terminal_input;
mod terminal_mouse;
mod terminal_view;

use std::borrow::Cow;

use gpui::{
    App, AppContext, Application, Bounds, Focusable, TitlebarOptions, WindowBounds, WindowOptions,
    px, size,
};
use mx_term::SpawnOptions;
use terminal_view::TerminalView;

const FONTS: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/Hack-Regular.ttf"),
    include_bytes!("../assets/fonts/Hack-Bold.ttf"),
    include_bytes!("../assets/fonts/Hack-Italic.ttf"),
    include_bytes!("../assets/fonts/Hack-BoldItalic.ttf"),
];

const DEMO_TERMINAL: &str = "MAESTRIA_DEMO_TERMINAL";

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    Application::new().with_assets(app::assets::Assets).run(|cx: &mut App| {
        if let Err(err) = cx
            .text_system()
            .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
        {
            log::error!("failed to load bundled fonts: {err}");
        }
        if let Ok(path) = std::env::var(reader::demo::ENV_PATH) {
            reader::demo::open(path, cx);
            return;
        }
        if let Some(component) = chrome::demo::requested() {
            chrome::demo::open(component, cx);
            return;
        }
        if let Ok(name) = std::env::var(rfw_view::DEMO_ENV) {
            rfw_view::open_demo(&name, cx);
            return;
        }
        if std::env::var_os(DEMO_TERMINAL).is_none() {
            if let Err(err) = app::open_main_window(cx) {
                log::error!("failed to open the main window: {err:#}");
                cx.quit();
            }
            return;
        }
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
        let spawn = SpawnOptions {
            cwd: std::env::current_dir().ok(),
            ..Default::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| match TerminalView::new(spawn, window, cx) {
                Ok(view) => view,
                Err(err) => {
                    log::error!("failed to spawn shell: {err:#}");
                    std::process::exit(1);
                }
            });
            window.focus(&view.focus_handle(cx));
            view
        });
        if let Err(err) = opened {
            log::error!("failed to open window: {err:#}");
            cx.quit();
        }
        cx.activate(true);
    });
}
