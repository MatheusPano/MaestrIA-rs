use gpui::{AppContext, Context, Entity, Window};
use mx_term::TermRegistry;
use mx_theme::TerminalPalette;

use crate::panes::PaneArea;
use crate::plugin_ui::surface::TermViews;
use crate::terminal_view::TerminalView;

pub struct Shown {
    pub id: String,
    pub claude: bool,
    pub cwd: String,
    pub type_size: (f32, f32),
}

#[derive(Default)]
pub struct Terminals {
    views: TermViews,
}

impl Terminals {
    pub fn get(&self, id: &str) -> Option<Entity<TerminalView>> {
        self.views.borrow().get(id).cloned()
    }

    pub fn all(&self) -> Vec<Entity<TerminalView>> {
        self.views.borrow().values().cloned().collect()
    }

    pub fn shared(&self) -> TermViews {
        self.views.clone()
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
                self.views.borrow_mut().remove(id);
                continue;
            };
            let current = self.get(id);
            let stale = current.as_ref().is_some_and(|v| !v.read(cx).session().same(&session));
            let view = match current {
                Some(v) if !stale => v,
                _ => {
                    let view = cx.new(|cx| {
                        let mut v = TerminalView::for_session(session, palette, window, cx);
                        v.set_claude(s.claude);
                        v.set_cwd(Some(s.cwd.clone()));
                        v
                    });
                    self.views.borrow_mut().insert(id.clone(), view.clone());
                    view
                }
            };
            let (size, line) = s.type_size;
            view.update(cx, |v, cx| v.set_typography(size, line, cx));
        }
        self.views.borrow_mut().retain(|id, _| shown.iter().any(|s| &s.id == id));
    }
}
