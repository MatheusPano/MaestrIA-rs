use std::rc::Rc;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use mx_claude::docs::MxDoc;
use mx_theme::Themes;

use super::{ReaderActions, ReaderView};

pub const ENV_PATH: &str = "MAESTRIA_DEMO_READER";
pub const ENV_THEME: &str = "MAESTRIA_DEMO_THEME";

pub fn open(path: String, cx: &mut App) {
    let theme = Themes::new().by_id(std::env::var(ENV_THEME).ok().as_deref());
    let bounds = Bounds::centered(None, size(px(1000.), px(760.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Maestria - leitor".into()),
            ..Default::default()
        }),
        app_id: Some("maestria".into()),
        ..Default::default()
    };
    let actions = ReaderActions {
        follow: Some(Rc::new(|href, base, _, _| log::info!("reader link: {href} from {base:?}"))),
        banner: Some(Rc::new(|text, _, _| log::info!("reader banner: {text}"))),
        reveal: Some(Rc::new(|path, _, _| log::info!("reader reveal: {path}"))),
        open_in_editor: Some(Rc::new(|path, _, _| log::info!("reader editor: {path}"))),
        toggle_pin: Some(Rc::new(|_, _| log::info!("reader pin"))),
        close: Some(Rc::new(|_, cx| cx.quit())),
    };
    let opened = cx.open_window(options, |_, cx| {
        cx.new(|cx| ReaderView::new(MxDoc::file(&path, None), theme, actions, cx))
    });
    if let Err(err) = opened {
        log::error!("failed to open reader demo: {err:#}");
        cx.quit();
    }
}
