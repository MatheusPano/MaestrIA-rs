use std::collections::HashMap;

use gpui::{AppContext, Context, Entity, Window};
use mx_term::TermRegistry;
use mx_theme::TerminalPalette;

use crate::panes::PaneArea;
use crate::terminal_view::TerminalView;

pub struct Shown {
    pub id: String,
    pub claude: bool,
    pub cwd: String,
    pub type_size: (f32, f32),
}

#[derive(Default)]
pub struct Terminals {
    views: HashMap<String, Entity<TerminalView>>,
}

impl Terminals {
    pub fn get(&self, id: &str) -> Option<&Entity<TerminalView>> {
        self.views.get(id)
    }

    pub fn sync(
        &mut self,
        shown: &[Shown],
        registry: &TermRegistry,
        palette: &TerminalPalette,
        window: &mut Window,
        cx: &mut Context<PaneArea>,
    ) {
        for s in shown {
            let id = &s.id;
            let Some(session) = registry.get(id) else {
                self.views.remove(id);
                continue;
            };
            let stale = self
                .views
                .get(id)
                .is_some_and(|v| !v.read(cx).session().same(&session));
            if stale || !self.views.contains_key(id) {
                let view = cx.new(|cx| {
                    let mut v = TerminalView::for_session(session, palette, window, cx);
                    v.set_claude(s.claude);
                    v.set_cwd(Some(s.cwd.clone()));
                    v
                });
                self.views.insert(id.clone(), view);
            }
            if let Some(view) = self.views.get(id) {
                let (size, line) = s.type_size;
                view.update(cx, |v, cx| v.set_typography(size, line, cx));
            }
        }
        self.views.retain(|id, _| shown.iter().any(|s| &s.id == id));
    }
}
