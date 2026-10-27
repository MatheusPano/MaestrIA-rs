use std::collections::HashMap;
use std::rc::Rc;

use gpui::{AnyElement, App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Styled, Window, div};
use mx_claude::docs::{DocSource, MxDoc};
use mx_claude::setup::MxSetup;
use mx_core::model::{Doc, Tab};

use crate::app::host::{StoreHost, update_store};
use crate::panes::PaneArea;
use crate::plugin_ui::surface::{Surface, SurfaceEnv};
use crate::reader::{ReaderActions, ReaderView};
use crate::rfw_view::BlocksMode;
use crate::setup::{SetupActions, SetupView};

pub fn mx_doc(d: &Doc) -> MxDoc {
    MxDoc {
        source: DocSource::from_name(d.source.name()).unwrap_or(DocSource::File),
        title: d.title.clone(),
        text: d.text.clone(),
        path: d.path.clone(),
        origin: d.origin.clone(),
        at: d.at.naive_local(),
    }
}

fn reader_actions(host: &Entity<StoreHost>, tab: &str) -> ReaderActions {
    let act = |f: fn(&mut mx_store::Store, &str)| {
        let host = host.clone();
        Rc::new(move |arg: String, _: &mut Window, cx: &mut App| update_store(&host, cx, |s| f(s, &arg))) as crate::reader::PathAction
    };
    let (follow_host, pin_host, close_host) = (host.clone(), host.clone(), host.clone());
    let (follow_tab, pin_tab, close_tab) = (tab.to_owned(), tab.to_owned(), tab.to_owned());
    ReaderActions {
        follow: Some(Rc::new(move |href, base, _, cx| {
            let tab = follow_tab.clone();
            update_store(&follow_host, cx, |s| s.follow_link(&href, Some(&tab), base.as_deref()));
        })),
        banner: Some(act(|s, text| s.show_banner(text))),
        reveal: Some(act(|s, path| s.reveal_file(path))),
        open_in_editor: Some(act(|s, path| s.open_in_editor(path))),
        toggle_pin: Some(Rc::new(move |_, cx| update_store(&pin_host, cx, |s| s.toggle_pin(&pin_tab)))),
        close: Some(Rc::new(move |_, cx| {
            update_store(&close_host, cx, |s| {
                s.close_tab(&close_tab);
            })
        })),
    }
}

struct Reading {
    view: Entity<ReaderView>,
    doc: Doc,
}

#[derive(Default)]
pub struct Bodies {
    readers: HashMap<String, Reading>,
    surfaces: HashMap<String, Surface>,
    setups: HashMap<String, Entity<SetupView>>,
}

impl Bodies {
    pub fn sync(&mut self, env: &SurfaceEnv, window: &mut Window, cx: &mut Context<PaneArea>) {
        let store = &env.host.read(cx).store;
        let open: Vec<Tab> = store.open_panes().into_iter().cloned().collect();
        let home = store.options().home.clone();
        let alive: Vec<String> = store.tabs().iter().filter(|t| t.is_setup()).map(|t| t.id.clone()).collect();
        let mut views = Vec::new();
        for t in &open {
            if let Some(v) = store.plugin_view(&t.id) {
                views.push((t.id.clone(), v.clone()));
            }
        }
        let titles: Vec<(String, Option<String>)> =
            open.iter().map(|t| (t.id.clone(), t.custom_label.as_ref().map(|_| store.title(t)))).collect();
        for t in open.iter().filter(|t| t.is_reader()) {
            let Some(doc) = t.doc.clone() else { continue };
            let title = titles.iter().find(|(id, _)| *id == t.id).and_then(|(_, l)| l.clone());
            self.sync_reader(env, t, doc, title, home.clone(), cx);
        }
        self.readers.retain(|id, _| open.iter().any(|t| t.is_reader() && &t.id == id));
        for (id, view) in &views {
            let surface = self.surfaces.entry(id.clone()).or_insert_with(|| Surface::new(id, BlocksMode::Framed));
            let mut env = env.clone();
            env.zoom = open.iter().find(|t| &t.id == id).map_or(0.0, |t| t.zoom as f32);
            surface.sync(view, &env, cx);
        }
        self.surfaces.retain(|id, _| views.iter().any(|(v, _)| v == id));
        self.sync_setups(env, &open, &alive, window, cx);
    }

    fn sync_reader(
        &mut self,
        env: &SurfaceEnv,
        t: &Tab,
        doc: Doc,
        title: Option<String>,
        home: Option<String>,
        cx: &mut Context<PaneArea>,
    ) {
        let theme = env.theme.clone();
        match self.readers.get_mut(&t.id) {
            Some(r) => {
                let changed = r.doc != doc;
                if changed {
                    r.doc = doc.clone();
                }
                let (pinned, zoom, cwd) = (t.pinned, t.zoom as f32, t.cwd.clone());
                r.view.update(cx, |v, cx| {
                    if changed {
                        v.set_doc(mx_doc(&doc), cx);
                    }
                    v.set_theme(theme, cx);
                    v.set_header(title, None, cx);
                    v.set_pinned(pinned, cx);
                    v.set_zoom(zoom, cx);
                    v.set_paths(Some(cwd), home);
                });
            }
            None => {
                let actions = reader_actions(&env.host, &t.id);
                let (pinned, zoom, cwd) = (t.pinned, t.zoom as f32, t.cwd.clone());
                let first = mx_doc(&doc);
                let view = cx.new(|cx| {
                    let mut v = ReaderView::new(first, theme, actions, cx);
                    v.set_header(title, None, cx);
                    v.set_pinned(pinned, cx);
                    v.set_zoom(zoom, cx);
                    v.set_paths(Some(cwd), home);
                    v
                });
                self.readers.insert(t.id.clone(), Reading { view, doc });
            }
        }
    }

    fn sync_setups(&mut self, env: &SurfaceEnv, open: &[Tab], alive: &[String], window: &mut Window, cx: &mut Context<PaneArea>) {
        for t in open.iter().filter(|t| t.is_setup()) {
            let Some(setup) = t.setup.clone() else { continue };
            let mx_setup = MxSetup { root: setup.root.clone(), selected: setup.selected.clone() };
            let (theme, pinned, zoom) = (env.theme.clone(), t.pinned, t.zoom as f32);
            let title = t.custom_label.clone();
            match self.setups.get_mut(&t.id) {
                Some(view) => {
                    view.update(cx, |v, cx| {
                        if *v.setup() != mx_setup {
                            v.set_setup(mx_setup, cx);
                        }
                        v.set_visible(true, cx);
                        v.set_theme(theme, cx);
                        v.set_pinned(pinned, cx);
                        v.set_zoom(zoom, cx);
                        v.set_title(title, cx);
                    });
                }
                None => {
                    let actions = setup_actions(&env.host, &t.id);
                    let view = cx.new(|cx| {
                        let mut v = SetupView::new(mx_setup, theme, actions, window, cx);
                        v.set_header(false, cx);
                        v.set_pinned(pinned, cx);
                        v.set_zoom(zoom, cx);
                        v.set_title(title, cx);
                        v
                    });
                    self.setups.insert(t.id.clone(), view);
                }
            }
        }
        let shown = |id: &String| open.iter().any(|t| t.is_setup() && &t.id == id);
        self.setups.retain(|id, view| shown(id) || (alive.contains(id) && view.read(cx).edited(cx)));
        for (id, view) in &self.setups {
            if !shown(id) {
                view.update(cx, |v, cx| v.set_visible(false, cx));
            }
        }
    }

    pub fn focus_target(&self, id: &str, cx: &App) -> Option<FocusHandle> {
        if let Some(r) = self.readers.get(id) {
            return Some(r.view.focus_handle(cx));
        }
        self.setups.get(id).map(|v| v.read(cx).focus_target(cx))
    }

    pub fn reader(&self, id: &str) -> Option<AnyElement> {
        self.readers.get(id).map(|r| r.view.clone().into_any_element())
    }

    pub fn surface(&self, id: &str) -> Option<AnyElement> {
        self.surfaces.get(id).map(Surface::element)
    }

    pub fn setup(&self, id: &str) -> Option<AnyElement> {
        self.setups.get(id).map(|v| v.clone().into_any_element())
    }
}

fn setup_actions(host: &Entity<StoreHost>, tab: &str) -> SetupActions {
    let path = |f: fn(&mut mx_store::Store, &str)| {
        let host = host.clone();
        Some(Rc::new(move |p: String, _: &mut Window, cx: &mut App| update_store(&host, cx, |s| f(s, &p))) as crate::setup::PathAction)
    };
    let on_tab = |f: fn(&mut mx_store::Store, &str)| {
        let (host, tab) = (host.clone(), tab.to_owned());
        Some(Rc::new(move |_: &mut Window, cx: &mut App| update_store(&host, cx, |s| f(s, &tab))) as crate::setup::Action)
    };
    let (banner_host, changed_host, changed_tab) = (host.clone(), host.clone(), tab.to_owned());
    SetupActions {
        banner: Some(Rc::new(move |text, sticky, _, cx| update_store(&banner_host, cx, |s| s.show_banner_with(&text, sticky)))),
        open_in_editor: path(|s, p| s.open_in_editor(p)),
        reveal: path(|s, p| s.reveal_file(p)),
        close: on_tab(|s, t| {
            s.close_tab(t);
        }),
        toggle_pin: on_tab(|s, t| s.toggle_pin(t)),
        changed: Some(Rc::new(move |setup: MxSetup, _, cx| {
            let tab = changed_tab.clone();
            update_store(&changed_host, cx, |s| {
                s.set_setup_selected(&tab, setup.selected.as_deref());
                s.touch();
            })
        })),
    }
}

pub fn placeholder(text: &'static str, color: gpui::Hsla) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(gpui::px(12.))
        .text_color(color)
        .child(text)
        .into_any_element()
}
