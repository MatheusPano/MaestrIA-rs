mod dialogs;
mod flow;
mod menus;
mod notices;
mod settings;

pub use notices::notices as demo_notices;
pub use settings::{data as settings_data, demo_validate};

use std::borrow::Cow;

use gpui::{
    AnyView, App, AppContext, Application, Bounds, Context, IntoElement, ParentElement, Render,
    Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size,
};

use crate::chrome::assets::ChromeAssets;
use crate::chrome::style::{hsla, set_theme, theme};

pub fn requested() -> Option<String> {
    std::env::var("MAESTRIA_DEMO_CHROME").ok().filter(|v| !v.is_empty())
}

fn window_size() -> (f32, f32) {
    std::env::var("MAESTRIA_DEMO_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1100., 720.))
}

pub fn run(component: String, fonts: Vec<Cow<'static, [u8]>>) {
    Application::new().with_assets(ChromeAssets).run(move |cx: &mut App| {
        if let Err(err) = cx.text_system().add_fonts(fonts) {
            log::error!("failed to load bundled fonts: {err}");
        }
        open(component, cx);
    });
}

pub fn open(component: String, cx: &mut App) {
    crate::chrome::register_fonts(cx);
    let id = std::env::var("MAESTRIA_DEMO_THEME").unwrap_or_default();
    set_theme(mx_theme::built_in(&id).cloned().unwrap_or_else(mx_theme::default_theme), cx);
    let (w, h) = window_size();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(w), px(h)), cx))),
        titlebar: Some(TitlebarOptions {
            title: Some(format!("chrome-demo {component}").into()),
            ..Default::default()
        }),
        app_id: Some("maestria-chrome-demo".into()),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        let scene = scene(&component, window, cx);
        cx.new(|_| DemoRoot { scene })
    });
    if let Err(err) = opened {
        log::error!("failed to open the demo window: {err:#}");
        cx.quit();
    }
}

fn scene(component: &str, window: &mut Window, cx: &mut App) -> AnyView {
    match component {
        c if c.starts_with("dialog") => dialogs::scene(c, window, cx),
        c if c.starts_with("settings") => settings::scene(c, window, cx),
        c if c.starts_with("flow") => flow::scene(c, window, cx),
        c if c.starts_with("notices") => notices::scene(c, window, cx),
        c => menus::scene(c, window, cx),
    }
}

struct DemoRoot {
    scene: AnyView,
}

impl Render for DemoRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        div()
            .size_full()
            .bg(hsla(t.canvas))
            .text_color(hsla(t.fg))
            .child(self.scene.clone())
    }
}
