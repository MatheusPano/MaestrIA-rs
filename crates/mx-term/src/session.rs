use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use alacritty_terminal::event::WindowSize;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty;
use mx_theme::TerminalPalette;

use crate::events::{Fanout, Subscriber};
use crate::ime::{TextInputGuard, TextOrigin};
use crate::io::{self, ChildPoll, Feeder, LifeIo, LifeOwner, Port, Proxy, SharedTerm};
use crate::keys::Mods;
use crate::mouse::{self, MouseAction, MouseButton};
use crate::paste::paste_bytes;
use crate::signals;
use crate::snapshot::{Snapshot, snapshot_into};
use crate::spawn::{self, Launch};
use crate::text;

pub const SCROLLBACK: usize = 8000;
const SUBMIT_DELAY: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermEvent {
    Wakeup,
    Title(String),
    ResetTitle,
    Bell,
    ClipboardStore(String),
    Exited(Option<i32>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}

impl GridSize {
    pub const DEFAULT: GridSize = GridSize { cols: 80, rows: 24, cell_width: 8, cell_height: 16 };

    pub(crate) fn window_size(self) -> WindowSize {
        WindowSize {
            num_lines: self.rows,
            num_cols: self.cols,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        usize::from(self.rows.max(1))
    }

    fn columns(&self) -> usize {
        usize::from(self.cols.max(2))
    }
}

#[derive(Clone, Debug, Default)]
pub struct SpawnOptions {
    pub program: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub scrollback: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    Cells,
    Word,
    Line,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteOutcome {
    Text,
    CtrlV,
    Nothing,
}

pub type ExitHook = Arc<dyn Fn(Option<i32>) + Send + Sync>;

#[derive(Default)]
struct State {
    generation: u64,
    life: Option<LifeIo>,
    pid: Option<u32>,
    started: bool,
    exited: bool,
    exit_code: Option<i32>,
}

pub(crate) struct Shared {
    pub(crate) port: Arc<Port>,
    pub(crate) term: SharedTerm,
    feeder: Arc<Mutex<Feeder>>,
    state: Mutex<State>,
    exit_cv: Condvar,
    guard: Mutex<TextInputGuard>,
    pub(crate) scrolled: AtomicBool,
    on_exit: Mutex<Option<ExitHook>>,
    tab_id: Mutex<Option<String>>,
    events: async_channel::Receiver<TermEvent>,
}

#[derive(Clone)]
pub struct TerminalSession {
    pub(crate) shared: Arc<Shared>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl TerminalSession {
    pub fn new(size: GridSize, palette: TerminalPalette, scrollback: usize) -> Self {
        let (fanout, rx) = Fanout::new();
        let port = Arc::new(Port {
            fanout,
            writer: Mutex::new(None),
            size: Mutex::new(size),
            palette: Mutex::new(Arc::new(palette)),
            sticky: Arc::default(),
        });
        let config = Config { scrolling_history: scrollback, ..Config::default() };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, Proxy(port.clone()))));
        port.sticky.attach(&term, &port);
        let shared = Arc::new(Shared {
            port,
            term,
            feeder: Arc::new(Mutex::new(Feeder::default())),
            state: Mutex::new(State::default()),
            exit_cv: Condvar::new(),
            guard: Mutex::new(TextInputGuard::new()),
            scrolled: AtomicBool::new(false),
            on_exit: Mutex::new(None),
            tab_id: Mutex::new(None),
            events: rx,
        });
        Self { shared }
    }

    pub fn with_palette(palette: TerminalPalette) -> Self {
        Self::new(GridSize::DEFAULT, palette, SCROLLBACK)
    }

    pub fn spawn(options: SpawnOptions, size: GridSize, palette: TerminalPalette) -> anyhow::Result<Self> {
        let session = Self::new(size, palette, options.scrollback.unwrap_or(SCROLLBACK));
        let launch = match options.program {
            Some(program) => Launch::Program { program, args: options.args },
            None => Launch::Shell,
        };
        session.start(launch, options.cwd, &options.env)?;
        Ok(session)
    }

    pub fn set_exit_hook(&self, hook: ExitHook) {
        *lock(&self.shared.on_exit) = Some(hook);
    }

    pub fn set_tab_id(&self, id: &str) {
        *lock(&self.shared.tab_id) = Some(id.to_owned());
    }

    pub fn tab_id(&self) -> Option<String> {
        lock(&self.shared.tab_id).clone()
    }

    pub fn events(&self) -> async_channel::Receiver<TermEvent> {
        self.shared.events.clone()
    }

    pub fn subscribe(&self) -> Subscriber {
        self.shared.port.fanout.subscribe()
    }

    pub fn same(&self, other: &TerminalSession) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    pub(crate) fn downgrade(&self) -> Weak<Shared> {
        Arc::downgrade(&self.shared)
    }

    pub(crate) fn upgrade(weak: &Weak<Shared>) -> Option<Self> {
        weak.upgrade().map(|shared| Self { shared })
    }

    pub fn start_shell_at(&self, cwd: &str) {
        self.start_checked(Launch::Shell, cwd);
    }

    pub fn start_command_at(&self, command: &str, cwd: &str, display: Option<&str>) {
        self.feed(spawn::note_line(display.unwrap_or(command)).as_bytes());
        self.start_checked(Launch::Command(command.to_owned()), cwd);
    }

    fn start_checked(&self, launch: Launch, cwd: &str) {
        {
            let mut st = lock(&self.shared.state);
            st.exited = false;
            st.exit_code = None;
        }
        if !cwd.is_empty() && !Path::new(cwd).is_dir() {
            self.feed(spawn::missing_cwd_line(cwd).as_bytes());
            self.finish_without_child(Some(-1));
            return;
        }
        let dir = (!cwd.is_empty()).then(|| PathBuf::from(cwd));
        if let Err(err) = self.start(launch, dir, &HashMap::new()) {
            self.feed(spawn::remark_line(&format!("{err:#}")).as_bytes());
            self.finish_without_child(Some(-1));
        }
    }

    fn finish_without_child(&self, code: Option<i32>) {
        {
            let mut st = lock(&self.shared.state);
            self.retire_life(&mut st);
            st.started = true;
            st.exited = true;
            st.exit_code = code;
        }
        self.shared.port.emit(TermEvent::Exited(code));
        let hook = lock(&self.shared.on_exit).clone();
        if let Some(hook) = hook {
            hook(code);
        }
    }

    fn start(&self, launch: Launch, cwd: Option<PathBuf>, env: &HashMap<String, String>) -> anyhow::Result<()> {
        let plan = spawn::plan(mx_sys::shell::shell(), mx_sys::shell::env(), &launch, cwd, env);
        let options = tty::Options {
            shell: Some(tty::Shell::new(plan.program, plan.args)),
            working_directory: plan.cwd,
            drain_on_exit: true,
            env: plan.env,
        };
        let size = self.size();
        let mut st = lock(&self.shared.state);
        self.retire_life(&mut st);
        let pty = tty::new(&options, size.window_size(), 0)?;
        st.started = true;
        st.exited = false;
        st.exit_code = None;
        st.pid = Some(pty.child().id());
        let owner: Weak<dyn LifeOwner> = Arc::downgrade(&self.shared) as Weak<dyn LifeOwner>;
        st.life = Some(io::start(
            pty,
            st.generation,
            self.shared.term.clone(),
            self.shared.port.clone(),
            self.shared.feeder.clone(),
            owner,
        )?);
        Ok(())
    }

    fn retire_life(&self, st: &mut State) {
        st.life = None;
        st.pid = None;
        st.generation += 1;
        lock(&self.shared.feeder).adopt(st.generation);
    }

    pub fn park_now(&self) {
        let mut st = lock(&self.shared.state);
        if !st.started {
            st.exited = true;
            st.exit_code = None;
        }
    }

    pub fn remark_now(&self, text: &str) {
        self.feed(spawn::remark_line(text).as_bytes());
    }

    pub fn relaunch_now(&self, command: &str, cwd: &str) {
        if !self.is_exited() {
            return;
        }
        {
            let mut st = lock(&self.shared.state);
            st.exited = false;
            st.exit_code = None;
        }
        self.feed(b"\r\n");
        self.start_command_at(command, cwd, None);
    }

    pub fn submit_text(&self, text: &str) {
        if self.is_exited() || text.trim().is_empty() {
            return;
        }
        self.paste(text);
        let generation = lock(&self.shared.state).generation;
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            std::thread::sleep(SUBMIT_DELAY);
            let st = lock(&shared.state);
            let alive = st.generation == generation && !st.exited && st.pid.is_some();
            drop(st);
            if alive {
                shared.port.write(b"\r");
            }
        });
    }

    pub fn kill_and_wait(&self) -> async_channel::Receiver<()> {
        let (tx, rx) = async_channel::bounded(1);
        let Some(generation) = ({
            let st = lock(&self.shared.state);
            st.pid.filter(|_| !st.exited).map(|_| st.generation)
        }) else {
            let _ = tx.try_send(());
            return rx;
        };
        let shared = self.shared.clone();
        std::thread::spawn(move || {
            let live = |s: &State| s.generation == generation && !s.exited;
            let st = lock(&shared.state);
            if let Some(pid) = st.pid.filter(|_| live(&st))
                && signals::hang_up(pid)
            {
                let (st, _) = shared
                    .exit_cv
                    .wait_timeout_while(st, signals::GRACE, |s| live(s))
                    .unwrap_or_else(|e| e.into_inner());
                if live(&st) {
                    signals::force_kill(pid);
                }
            }
            let _ = tx.try_send(());
        });
        rx
    }

    pub fn hang_up_now(&self) {
        let st = lock(&self.shared.state);
        if let Some(pid) = st.pid.filter(|_| !st.exited) {
            signals::hang_up(pid);
        }
    }

    pub fn child_pid(&self) -> Option<u32> {
        let st = lock(&self.shared.state);
        st.pid.filter(|_| !st.exited)
    }

    pub fn is_exited(&self) -> bool {
        lock(&self.shared.state).exited
    }

    pub fn exit_status(&self) -> Option<i32> {
        lock(&self.shared.state).exit_code
    }

    pub fn feed(&self, bytes: &[u8]) {
        lock(&self.shared.feeder).feed(&self.shared.term, &self.shared.port, bytes);
    }

    pub fn write(&self, bytes: &[u8]) {
        if self.shared.scrolled.swap(false, Ordering::AcqRel) {
            self.shared.term.lock().scroll_display(Scroll::Bottom);
            self.shared.port.wake();
        }
        self.shared.port.write(bytes);
    }

    pub fn send(&self, text: &str) {
        lock(&self.shared.guard).forget_last();
        self.write(text.as_bytes());
    }

    pub fn newline(&self) {
        self.send("\x1b\r");
    }

    pub fn paste(&self, text: &str) {
        lock(&self.shared.guard).forget_last();
        self.write(&paste_bytes(text, self.mode().contains(TermMode::BRACKETED_PASTE)));
    }

    pub fn paste_clipboard(&self, text: Option<&str>, images_via_ctrl_v: bool) -> PasteOutcome {
        match text.filter(|t| !t.is_empty()) {
            Some(t) => {
                self.paste(t);
                self.clear_selection();
                PasteOutcome::Text
            }
            None if images_via_ctrl_v => {
                self.send("\x16");
                PasteOutcome::CtrlV
            }
            None => PasteOutcome::Nothing,
        }
    }

    pub fn key_down(&self, key: &str) {
        lock(&self.shared.guard).key_down(key);
    }

    pub fn key_up(&self, key: &str) {
        lock(&self.shared.guard).key_up(key);
    }

    pub fn preedit(&self, text: &str) {
        lock(&self.shared.guard).preedit(text);
    }

    pub fn input_text(&self, text: &str, origin: TextOrigin) -> bool {
        let ok = lock(&self.shared.guard).accept(text, origin, Instant::now());
        if ok {
            self.write(text.as_bytes());
        }
        ok
    }

    pub fn size(&self) -> GridSize {
        *lock(&self.shared.port.size)
    }

    pub fn resize(&self, size: GridSize) {
        {
            let mut current = lock(&self.shared.port.size);
            if *current == size {
                return;
            }
            *current = size;
        }
        self.shared.term.lock().resize(size);
        self.shared.port.resize_pty(size);
    }

    pub fn set_palette(&self, palette: TerminalPalette) {
        *lock(&self.shared.port.palette) = Arc::new(palette);
        self.shared.port.wake();
    }

    pub fn scroll(&self, lines: i32) {
        let mut term = self.shared.term.lock();
        term.scroll_display(Scroll::Delta(lines));
        self.shared.scrolled.store(term.grid().display_offset() != 0, Ordering::Release);
        drop(term);
        self.shared.port.wake();
    }

    pub fn mode(&self) -> TermMode {
        *self.shared.term.lock().mode()
    }

    pub fn take_dirty(&self) -> bool {
        self.shared.port.fanout.take_dirty()
    }

    pub fn snapshot_into(&self, out: &mut Snapshot) {
        let palette = lock(&self.shared.port.palette).clone();
        snapshot_into(&self.shared.term.lock(), &palette, out);
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut snap = Snapshot::default();
        self.snapshot_into(&mut snap);
        snap
    }

    pub fn row_text(&self, row: usize) -> String {
        text::row_text(&self.shared.term.lock(), row)
    }

    pub fn wheel(&self, up: bool, col: usize, row: usize) {
        let bytes = mouse::wheel_bytes(up, col, row, self.mode());
        self.shared.port.write(&bytes);
    }

    pub fn mouse_report(&self, button: MouseButton, action: MouseAction, col: usize, row: usize, mods: Mods) -> bool {
        match mouse::button_report(button, action, col, row, mods, self.mode()) {
            Some(bytes) => {
                self.shared.port.write(&bytes);
                true
            }
            None => false,
        }
    }

    pub fn start_selection(&self, col: usize, row: usize, right_half: bool, kind: SelectionKind) {
        let mut term = self.shared.term.lock();
        let Some(point) = text::viewport_point(&term, col, row) else { return };
        let ty = match kind {
            SelectionKind::Cells => SelectionType::Simple,
            SelectionKind::Word => SelectionType::Semantic,
            SelectionKind::Line => SelectionType::Lines,
        };
        term.selection = Some(Selection::new(ty, point, side(right_half)));
        self.shared.port.sticky.selected(&term);
        drop(term);
        self.shared.port.wake();
    }

    pub fn update_selection(&self, col: usize, row: usize, right_half: bool) {
        let mut term = self.shared.term.lock();
        let Some(point) = text::viewport_point(&term, col, row) else { return };
        if let Some(sel) = term.selection.as_mut() {
            sel.update(point, side(right_half));
        }
        self.shared.port.sticky.selected(&term);
        drop(term);
        self.shared.port.wake();
    }

    pub fn select_all(&self) {
        let mut term = self.shared.term.lock();
        let top = Point::new(term.grid().topmost_line(), Column(0));
        let bottom = Point::new(term.grid().bottommost_line(), Column(term.columns() - 1));
        let mut sel = Selection::new(SelectionType::Simple, top, Side::Left);
        sel.update(bottom, Side::Right);
        term.selection = Some(sel);
        self.shared.port.sticky.selected(&term);
        drop(term);
        self.shared.port.wake();
    }

    pub fn clear_selection(&self) {
        let mut term = self.shared.term.lock();
        let had = term.selection.take().is_some();
        self.shared.port.sticky.selected(&term);
        drop(term);
        if had {
            self.shared.port.wake();
        }
    }

    pub fn has_selection(&self) -> bool {
        self.shared.term.lock().selection.as_ref().is_some_and(|s| !s.is_empty())
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection_text(false)
    }

    pub fn link_at(&self, col: usize, row: usize, base: Option<&str>) -> Option<String> {
        text::link_at_cell(&self.shared.term.lock(), col, row, base)
    }
}

fn side(right_half: bool) -> Side {
    if right_half { Side::Right } else { Side::Left }
}

impl LifeOwner for Shared {
    fn reap(&self, generation: u64, poll: ChildPoll<'_>) -> bool {
        let mut st = lock(&self.state);
        let Some(code) = poll() else { return false };
        if st.generation != generation || st.exited {
            return true;
        }
        st.exited = true;
        st.exit_code = code;
        st.pid = None;
        drop(st);
        self.exit_cv.notify_all();
        lock(&self.feeder).feed(&self.term, &self.port, spawn::exit_line(code).as_bytes());
        self.port.emit(TermEvent::Exited(code));
        let hook = lock(&self.on_exit).clone();
        if let Some(hook) = hook {
            hook(code);
        }
        true
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let st = lock(&self.state);
        if let Some(pid) = st.pid.filter(|_| !st.exited) {
            signals::hang_up(pid);
        }
    }
}
