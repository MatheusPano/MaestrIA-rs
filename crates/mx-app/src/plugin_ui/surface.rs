use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{AnyElement, AnyView, App, AppContext, Entity, IntoElement, ParentElement, Styled, div};
use mx_plugins::ui::view::PluginView;
use mx_theme::{Theme, Typography};

use crate::app::host::{StoreHost, update_store};
use crate::rfw_view::{ActionSink, BlocksMode, BlocksView, RfwView, ViewAction};
use crate::terminal_view::TerminalView;

pub type TermViews = Rc<RefCell<HashMap<String, Entity<TerminalView>>>>;

pub fn rfw_typography(t: &mx_core::persist::Typography) -> Typography {
    Typography::new(Some(&t.mono), Some(t.size), Some(t.line), &|_| true)
}

#[derive(Clone)]
pub struct SurfaceEnv {
    pub host: Entity<StoreHost>,
    pub theme: Arc<Theme>,
    pub typography: Typography,
    pub terms: TermViews,
    pub zoom: f32,
}

impl SurfaceEnv {
    pub fn dir_of(&self, plugin: &str, cx: &App) -> Option<PathBuf> {
        let store = &self.host.read(cx).store;
        store.plugins()?.get(plugin).map(|p| p.dir)
    }
}

pub fn sink_for(host: &Entity<StoreHost>, tab: &str) -> ActionSink {
    let host = host.clone();
    let tab = tab.to_owned();
    Rc::new(move |a: ViewAction, _, cx: &mut App| {
        update_store(&host, cx, |s| s.view_action(&tab, &a.action, a.values));
    })
}

pub struct Surface {
    tab: String,
    mode: BlocksMode,
    rfw: Option<Entity<RfwView>>,
    blocks: Option<Entity<BlocksView>>,
    revision: Option<u64>,
    rfw_revision: Option<u64>,
    consoles: Vec<(String, u64)>,
    theme: Option<Arc<Theme>>,
    zoom: f32,
}

impl Surface {
    pub fn new(tab: &str, mode: BlocksMode) -> Surface {
        Surface {
            tab: tab.to_owned(),
            mode,
            rfw: None,
            blocks: None,
            revision: None,
            rfw_revision: None,
            consoles: Vec::new(),
            theme: None,
            zoom: 0.0,
        }
    }

    pub fn sync(&mut self, view: &PluginView, env: &SurfaceEnv, cx: &mut App) {
        let restyled = !self.theme.as_ref().is_some_and(|t| Arc::ptr_eq(t, &env.theme));
        self.theme = Some(env.theme.clone());
        if view.uses_rfw() {
            self.blocks = None;
            self.revision = None;
            self.sync_rfw(view, env, restyled, cx);
        } else {
            self.rfw = None;
            self.rfw_revision = None;
            self.sync_blocks(view, env, restyled, cx);
        }
    }

    fn sync_rfw(&mut self, view: &PluginView, env: &SurfaceEnv, restyled: bool, cx: &mut App) {
        let entity = match &self.rfw {
            Some(e) => e.clone(),
            None => {
                let dir = env.dir_of(&view.plugin_id, cx);
                let sink = sink_for(&env.host, &self.tab);
                let (theme, typo) = (env.theme.clone(), env.typography.clone());
                let e = cx.new(|_| RfwView::new(theme, &typo, dir, sink));
                self.rfw = Some(e.clone());
                self.rfw_revision = None;
                e
            }
        };
        let fresh = self.rfw_revision != Some(view.rfw_revision);
        self.rfw_revision = Some(view.rfw_revision);
        let lib = view.rfw_library.clone().unwrap_or_default();
        let root = view.rfw_root.clone();
        let data = view.rfw_data.clone();
        let (theme, typo) = (env.theme.clone(), env.typography.clone());
        entity.update(cx, |v, cx| {
            v.set_library(&lib);
            v.set_root(&root);
            if fresh {
                v.merge_data(&data);
            }
            if restyled {
                v.set_theme(theme, &typo);
            }
            cx.notify();
        });
    }

    fn sync_blocks(&mut self, view: &PluginView, env: &SurfaceEnv, restyled: bool, cx: &mut App) {
        let entity = match &self.blocks {
            Some(e) => e.clone(),
            None => {
                let dir = env.dir_of(&view.plugin_id, cx);
                let sink = sink_for(&env.host, &self.tab);
                let (theme, typo, mode) = (env.theme.clone(), env.typography.clone(), self.mode);
                let terms = env.terms.clone();
                let e = cx.new(|_| {
                    let mut b = BlocksView::new(theme, &typo, dir, mode, sink);
                    b.set_terminal_slot(Some(Box::new(move |id: &str, _: &mut App| {
                        terms.borrow().get(id).map(|t| AnyView::from(t.clone()))
                    })));
                    b
                });
                self.blocks = Some(e.clone());
                self.revision = None;
                self.consoles.clear();
                e
            }
        };
        let fresh = self.revision != Some(view.revision);
        self.revision = Some(view.revision);
        let consoles: Vec<(String, u64)> = view.consoles.iter().map(|(k, c)| (k.clone(), c.revision)).collect();
        let lines = consoles != self.consoles;
        self.consoles = consoles;
        let zoomed = self.zoom != env.zoom;
        self.zoom = env.zoom;
        if !(fresh || lines || restyled || zoomed) {
            return;
        }
        let theme = env.theme.clone();
        let zoom = env.zoom;
        entity.update(cx, |v, cx| {
            if lines || fresh {
                v.set_consoles(view.consoles.clone());
            }
            if restyled {
                v.set_theme(theme);
            }
            v.set_zoom(zoom);
            if fresh {
                v.set_blocks(view.blocks.clone(), cx);
            }
            cx.notify();
        });
    }

    pub fn element(&self) -> AnyElement {
        if let Some(r) = &self.rfw {
            return div().size_full().child(r.clone()).into_any_element();
        }
        if let Some(b) = &self.blocks {
            return div().size_full().child(b.clone()).into_any_element();
        }
        div().size_full().into_any_element()
    }
}
