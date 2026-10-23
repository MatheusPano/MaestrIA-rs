use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use mx_core::ports::{BoxFuture, Term, TermFactory};
use mx_theme::TerminalPalette;

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
    on_exit: Option<TabExitHook>,
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
                on_exit: None,
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

    pub fn create_session(&self, tab_id: &str) -> TerminalSession {
        let (palette, hook) = match self.inner.lock() {
            Ok(r) => (r.palette.clone(), r.on_exit.clone()),
            Err(_) => (TerminalPalette::default(), None),
        };
        let session = TerminalSession::with_palette(palette);
        session.set_tab_id(tab_id);
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
}
