use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use mx_core::persist::CursorShape;
use mx_core::ports::{BoxFuture, Term, TermFactory};
use mx_theme::TerminalPalette;

use crate::ptyd::Ptyd;
use crate::session::{Shared, TerminalSession};

impl Term for TerminalSession {
    fn start_shell(&mut self, cwd: &str) {
        self.start_shell_at(cwd);
    }

    fn start_command(&mut self, command: &str, cwd: &str, display: Option<&str>) {
        self.start_command_at(command, cwd, display);
    }

    fn park(&mut self) {
        self.park_now();
    }

    fn remark(&mut self, text: &str) {
        self.remark_now(text);
    }

    fn kill(&mut self) -> BoxFuture<'static, ()> {
        let done = self.kill_and_wait();
        Box::pin(async move {
            let _ = done.recv().await;
        })
    }

    fn hang_up(&mut self) {
        self.hang_up_now();
    }

    fn keep_alive(&mut self) {
        TerminalSession::keep_alive(self);
    }

    fn slept_away(&self) -> bool {
        TerminalSession::slept_away(self)
    }

    fn relaunch(&mut self, command: &str, cwd: &str) {
        self.relaunch_now(command, cwd);
    }

    fn submit(&mut self, text: &str) {
        self.submit_text(text);
    }

    fn pid(&self) -> Option<u32> {
        self.child_pid()
    }

    fn exited(&self) -> bool {
        self.is_exited()
    }

    fn exit_code(&self) -> Option<i32> {
        self.exit_status()
    }
}

pub type TabExitHook = Arc<dyn Fn(&str, Option<i32>) + Send + Sync>;

struct Registry {
    sessions: HashMap<String, Weak<Shared>>,
    palette: TerminalPalette,
    cursor: CursorShape,
    on_exit: Option<TabExitHook>,
    ptyd: Option<Arc<Ptyd>>,
}

#[derive(Clone)]
pub struct TermRegistry {
    inner: Arc<Mutex<Registry>>,
}

impl TermRegistry {
    pub fn new(palette: TerminalPalette) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Registry {
                sessions: HashMap::new(),
                palette,
                cursor: CursorShape::Block,
                on_exit: None,
                ptyd: None,
            })),
        }
    }

    pub fn on_exit(&self, hook: TabExitHook) {
        if let Ok(mut r) = self.inner.lock() {
            r.on_exit = Some(hook);
        }
    }

    pub fn get(&self, tab_id: &str) -> Option<TerminalSession> {
        let mut r = self.inner.lock().ok()?;
        let found = r.sessions.get(tab_id).and_then(TerminalSession::upgrade);
        if found.is_none() {
            r.sessions.remove(tab_id);
        }
        found
    }

    pub fn set_palette(&self, palette: TerminalPalette) {
        let Ok(mut r) = self.inner.lock() else { return };
        r.sessions.retain(|_, w| w.strong_count() > 0);
        for weak in r.sessions.values() {
            if let Some(s) = TerminalSession::upgrade(weak) {
                s.set_palette(palette.clone());
            }
        }
        r.palette = palette;
    }

    pub fn set_cursor_shape(&self, shape: CursorShape) {
        let Ok(mut r) = self.inner.lock() else { return };
        if r.cursor == shape {
            return;
        }
        r.cursor = shape;
        r.sessions.retain(|_, w| w.strong_count() > 0);
        for weak in r.sessions.values() {
            if let Some(s) = TerminalSession::upgrade(weak) {
                s.set_cursor_shape(shape);
            }
        }
    }

    pub fn use_ptyd(&self, ptyd: Option<Ptyd>) {
        if let Ok(mut r) = self.inner.lock() {
            r.ptyd = ptyd.map(Arc::new);
        }
    }

    pub fn ptyd(&self) -> Option<Arc<Ptyd>> {
        self.inner.lock().ok().and_then(|r| r.ptyd.clone())
    }

    pub fn hibernate_when_idle(&self, claudes: Vec<String>, shells: Vec<String>, after: std::time::Duration) -> std::io::Result<()> {
        match self.ptyd() {
            Some(ptyd) if !claudes.is_empty() || !shells.is_empty() => ptyd.hibernate_when_idle(claudes, shells, after),
            _ => Ok(()),
        }
    }

    pub fn prune_ptyd(&self) -> std::io::Result<usize> {
        let Some(ptyd) = self.ptyd() else { return Ok(0) };
        let keep: Vec<String> = match self.inner.lock() {
            Ok(r) => r.sessions.iter().filter(|(_, w)| w.strong_count() > 0).map(|(k, _)| k.clone()).collect(),
            Err(_) => return Ok(0),
        };
        ptyd.sweep_finished(&keep);
        ptyd.prune(keep)
    }

    pub fn adopt_session(&self, tab_id: &str) -> Option<TerminalSession> {
        let ptyd = self.ptyd()?;
        let session = self.create_session(tab_id);
        if session.reattach() {
            return Some(session);
        }
        session.restore_finished(ptyd.take_finished(tab_id)?);
        Some(session)
    }

    pub fn create_session(&self, tab_id: &str) -> TerminalSession {
        let (palette, cursor, hook, ptyd) = match self.inner.lock() {
            Ok(r) => (r.palette.clone(), r.cursor, r.on_exit.clone(), r.ptyd.clone()),
            Err(_) => (TerminalPalette::default(), CursorShape::Block, None, None),
        };
        let session = TerminalSession::with_palette(palette);
        if cursor != CursorShape::Block {
            session.set_cursor_shape(cursor);
        }
        session.set_tab_id(tab_id);
        session.set_ptyd(ptyd);
        if let Some(hook) = hook {
            let id = tab_id.to_owned();
            session.set_exit_hook(Arc::new(move |code| hook(&id, code)));
        }
        if let Ok(mut r) = self.inner.lock() {
            r.sessions.retain(|_, w| w.strong_count() > 0);
            r.sessions.insert(tab_id.to_owned(), session.downgrade());
        }
        session
    }
}

impl TermFactory for TermRegistry {
    fn create(&self, tab_id: &str) -> Box<dyn Term> {
        Box::new(self.create_session(tab_id))
    }

    fn adopt(&self, tab_id: &str) -> Option<Box<dyn Term>> {
        self.adopt_session(tab_id).map(|s| Box::new(s) as Box<dyn Term>)
    }
}
