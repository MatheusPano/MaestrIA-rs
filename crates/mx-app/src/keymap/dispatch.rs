use gpui::{App, Entity, KeyContext, KeyDownEvent, Window};
use mx_plugins::Plugins;
use mx_plugins::chord::KeyPress;
use mx_store::{ClaudeSpec, DocPlace, Store};

use super::action::Action;
use super::chord::{LINUX, is_editing_key, may_be_shortcut, press_of, terminal_key};
use super::map::Keymap;
use crate::app::host::{StoreHost, update_store};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Action(Action),
    Panel(String),
    Plugin(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focus {
    pub terminal: bool,
    pub text_field: bool,
    pub own_find: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiRequest {
    Settings,
    Rename { tab_id: String, title: String },
    NewTask { folder: String },
    FocusSearch,
    FindInPane { tab_id: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Done,
    Ui(UiRequest),
}

pub const NO_FOLDER_FOR_TASK: &str = "Nova task precisa de uma pasta — o painel em foco não está em nenhuma";
pub const NOTHING_TO_RENAME: &str = "Nenhum painel em foco pra renomear";
pub const NOTHING_TO_PIN: &str = "Nenhum painel em foco pra prender";
pub const NOT_CLAUDE: &str = "O painel em foco não é uma sessão do Claude";

pub fn resolve(
    map: &Keymap,
    p: &KeyPress,
    focus: Focus,
    panel: impl FnOnce(&KeyPress) -> Option<String>,
    plugin: impl FnOnce(&KeyPress) -> Option<String>,
) -> Option<Hit> {
    if !may_be_shortcut(p) || (focus.terminal && terminal_key(p, map.linux()).is_some()) {
        return None;
    }
    let action = map.matching(p);
    if focus.own_find && action == Some(Action::FindInPane) {
        return None;
    }
    if focus.text_field && is_editing_key(p.key) {
        return if action.is_some() { None } else { plugin(p).map(Hit::Plugin) };
    }
    if let Some(a) = action {
        return Some(Hit::Action(a));
    }
    panel(p).map(Hit::Panel).or_else(|| plugin(p).map(Hit::Plugin))
}

pub fn panel_hit(store: &Store, p: &KeyPress) -> Option<String> {
    store.panel_for(|id| mx_plugins::chord::Chord::parse(id).is_some_and(|c| c.accepts(p)))
}

pub fn plugin_hit(plugins: &Plugins, p: &KeyPress, linux: bool) -> Option<String> {
    let hit = plugins
        .commands()
        .into_iter()
        .filter(|c| c.key.is_some_and(|k| k.hits(p)))
        .min_by(|a, b| a.plugin_id.cmp(&b.plugin_id))?;
    if let Some(k) = hit.key {
        plugins.note(&hit.plugin_id, &format!("tecla {} → {}", k.label(linux), hit.id));
    }
    Some(hit.full_id())
}

pub fn keymap_of(store: &Store) -> Keymap {
    Keymap::from_obj(&store.prefs().shortcuts, LINUX)
}

pub fn run(store: &mut Store, hit: &Hit) -> Option<UiRequest> {
    match hit {
        Hit::Action(a) => run_action(store, *a),
        Hit::Panel(id) => {
            store.select(id);
            None
        }
        Hit::Plugin(full_id) => {
            store.run_plugin_command(full_id, None);
            None
        }
    }
}

fn focused_feature(store: &Store) -> Option<String> {
    store.focused_feature().map(|f| f.id.clone())
}

pub fn run_action(store: &mut Store, a: Action) -> Option<UiRequest> {
    let focused = store.focused_tab_id();
    match a {
        Action::Claude => {
            let folder = store.focused_folder().clone();
            let mut spec = ClaudeSpec::new(&folder, &folder.root);
            spec.feature = focused_feature(store);
            store.open_claude(spec);
        }
        Action::Shell => {
            let folder = store.focused_folder().clone();
            let feature = focused_feature(store);
            store.open_shell(&folder, None, None, feature.as_deref(), None);
        }
        Action::Task => {
            let folder = store.focused_folder();
            if folder.is_loose {
                store.show_banner(NO_FOLDER_FOR_TASK);
            } else {
                return Some(UiRequest::NewTask { folder: folder.root.clone() });
            }
        }
        Action::Rename => match store.focused_tab() {
            None => store.show_banner(NOTHING_TO_RENAME),
            Some(t) => return Some(UiRequest::Rename { tab_id: t.id.clone(), title: store.title(t) }),
        },
        Action::Pin => match focused {
            None => store.show_banner(NOTHING_TO_PIN),
            Some(id) => store.toggle_pin(&id),
        },
        Action::Close => store.close_focused(),
        Action::CloseSettled => {
            let root = store.focused_folder().root.clone();
            store.close_settled(&root);
        }
        Action::NextPane => store.cycle_pane(1),
        Action::PrevPane => store.cycle_pane(-1),
        Action::NextSession => store.cycle(1),
        Action::PrevSession => store.cycle(-1),
        Action::FindInPane => return Some(UiRequest::FindInPane { tab_id: focused }),
        Action::Search => {
            if store.prefs().sidebar_hidden || store.prefs().sidebar_view.is_some() {
                store.show_sidebar_view(None);
            }
            return Some(UiRequest::FocusSearch);
        }
        Action::Markdown => store.open_markdown(DocPlace::default()),
        Action::Plan => match store.focused_tab() {
            Some(t) if t.kind == mx_core::model::TabKind::Claude => {
                let id = t.id.clone();
                store.show_plan(&id, None);
            }
            _ => store.show_banner(NOT_CLAUDE),
        },
        Action::Refresh => store.refresh_git(),
        Action::ZoomIn => store.zoom_focused(1),
        Action::ZoomOut => store.zoom_focused(-1),
        Action::ZoomReset => store.reset_zoom_focused(),
        Action::Sidebar => store.toggle_sidebar(),
        Action::Settings => return Some(UiRequest::Settings),
    }
    None
}

const TEXT_CONTEXTS: [&str; 4] = ["TextField", "SidebarSearch", "SetupEditor", "CodeEditor"];

pub fn focus_of(window: &Window) -> Focus {
    focus_from(&window.context_stack())
}

pub fn focus_from(stack: &[KeyContext]) -> Focus {
    let mut f = Focus::default();
    for ctx in stack {
        f.terminal |= ctx.contains("Terminal");
        f.text_field |= TEXT_CONTEXTS.iter().any(|name| ctx.contains(name));
        f.own_find |= ctx.contains("CodeEditor");
    }
    f
}

pub fn on_key_down(host: &Entity<StoreHost>, ev: &KeyDownEvent, window: &Window, cx: &mut App) -> Outcome {
    let Some(p) = press_of(&ev.keystroke) else { return Outcome::Pass };
    if !may_be_shortcut(&p) {
        return Outcome::Pass;
    }
    let focus = focus_of(window);
    let hit = {
        let store = &host.read(cx).store;
        let map = keymap_of(store);
        resolve(&map, &p, focus, |p| panel_hit(store, p), |p| store.plugins().and_then(|r| plugin_hit(r, p, LINUX)))
    };
    let Some(hit) = hit else { return Outcome::Pass };
    cx.stop_propagation();
    match update_store(host, cx, |s| run(s, &hit)) {
        Some(ui) => Outcome::Ui(ui),
        None => Outcome::Done,
    }
}
