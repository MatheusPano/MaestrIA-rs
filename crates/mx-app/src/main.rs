#![forbid(unsafe_code)]

mod terminal_element;
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

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    Application::new().run(|cx: &mut App| {
        if let Err(err) = cx
            .text_system()
            .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
        {
            log::error!("failed to load bundled fonts: {err}");
        }
        let bounds = Bounds::centered(None, size(px(960.), px(600.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Maestria".into()),
                ..Default::default()
            }),
            app_id: Some("maestria".into()),
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
