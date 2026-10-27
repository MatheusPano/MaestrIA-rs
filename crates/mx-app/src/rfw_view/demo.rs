use std::path::PathBuf;
use std::rc::Rc;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use mx_theme::{Typography, default_theme};

use super::view::RfwView;
use super::{ActionSink, ViewAction};

pub const DEMO_ENV: &str = "MAESTRIA_DEMO_RFW";

struct Demo {
    file: &'static str,
    data: &'static str,
    root: &'static str,
    plugin: &'static str,
    width: f32,
    height: f32,
}

fn demo(name: &str) -> Option<Demo> {
    let d = |file, data, root, plugin, width, height| Demo { file, data, root, plugin, width, height };
    Some(match name {
        "argo" => d("maestria-plugins__argocd__argo.rfwtxt", "argo.json", "home", "argocd", 900.0, 600.0),
        "argo-tree" => d("maestria-plugins__argocd__argo.rfwtxt", "argo.json", "tree", "argocd", 900.0, 600.0),
        "kanban" => d("maestria-plugins__wiboor__kanban.rfwtxt", "kanban.json", "root", "wiboor", 1100.0, 680.0),
        "devtools" => d("maestria-plugins__flutter__devtools.rfwtxt", "devtools.json", "root", "flutter", 900.0, 640.0),
        "docker" => d("maestria-plugins__docker__sidebar.rfwtxt", "docker_sidebar.json", "root", "docker", 300.0, 640.0),
        "ssh" => d("maestria-plugins__ssh__hosts.rfwtxt", "ssh_hosts.json", "root", "ssh", 820.0, 560.0),
        "pomodoro" => d("maestria-plugins__pomodoro__sidebar.rfwtxt", "pomodoro_sidebar.json", "root", "pomodoro", 300.0, 720.0),
        "pomodoro-float" => d("maestria-plugins__pomodoro__float.rfwtxt", "pomodoro_float.json", "root", "pomodoro", 200.0, 64.0),
        _ => return None,
    })
}

fn app_dir() -> PathBuf {
    std::env::var_os("MAESTRIA_DEMO_APP_DIR").map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
}

fn fixtures() -> PathBuf {
    app_dir().join("../mx-rfw/tests/fixtures")
}

pub(super) fn window_options(title: &str, width: f32, height: f32, cx: &mut App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(width), px(height)), cx))),
        titlebar: Some(TitlebarOptions { title: Some(title.to_owned().into()), ..Default::default() }),
        app_id: Some("maestria-demo".into()),
        ..Default::default()
    }
}

pub(super) fn logging_sink() -> ActionSink {
    Rc::new(|a: ViewAction, _, _| log::info!("view.action {} {}", a.action, serde_json::Value::Object(a.values)))
}

pub fn open_demo(name: &str, cx: &mut App) {
    if let Some(rest) = name.strip_prefix("blocks") {
        super::blocks::open_demo(rest.trim_start_matches('@').parse().unwrap_or(0), cx);
        return;
    }
    let Some(d) = demo(name) else {
        log::error!("{DEMO_ENV}: unknown fixture {name}");
        cx.quit();
        return;
    };
    let library = std::fs::read_to_string(fixtures().join(d.file)).unwrap_or_default();
    let data: serde_json::Value = std::fs::read_to_string(fixtures().join("data").join(d.data))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let dir = app_dir().join("../../../maestria-plugins").join(d.plugin);
    let dir = dir.is_dir().then_some(dir);
    let options = window_options(&format!("rfw · {name}"), d.width, d.height, cx);
    let opened = cx.open_window(options, |_, cx| {
        cx.new(|_| {
            let mut view = RfwView::new(default_theme().clone(), &Typography::standard(), dir, logging_sink());
            view.set_background(Some(super::style::hsla(default_theme().bg)));
            view.set_library(&library);
            view.set_root(d.root);
            if let Some(obj) = data.as_object() {
                view.merge_data(obj);
            }
            view
        })
    });
    if let Err(err) = opened {
        log::error!("failed to open demo window: {err:#}");
        cx.quit();
    }
}
