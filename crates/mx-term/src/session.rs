use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::Rgb as AnsiRgb;
use mx_theme::TerminalPalette;
use std::sync::Mutex;

use crate::snapshot::{Snapshot, snapshot};

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
    fn window_size(self) -> WindowSize {
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

#[derive(Clone)]
struct Proxy {
    events: async_channel::Sender<TermEvent>,
    pty: Arc<Mutex<Option<EventLoopSender>>>,
    size: Arc<Mutex<GridSize>>,
    palette: Arc<TerminalPalette>,
}

impl Proxy {
    fn write(&self, text: String) {
        if let Some(sender) = self.pty.lock().ok().and_then(|g| g.clone()) {
            let _ = sender.send(Msg::Input(Cow::Owned(text.into_bytes())));
        }
    }

    fn emit(&self, event: TermEvent) {
        let _ = self.events.try_send(event);
    }
}

impl EventListener for Proxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup => self.emit(TermEvent::Wakeup),
            Event::Title(t) => self.emit(TermEvent::Title(t)),
            Event::ResetTitle => self.emit(TermEvent::ResetTitle),
            Event::Bell => self.emit(TermEvent::Bell),
            Event::ClipboardStore(_, text) => self.emit(TermEvent::ClipboardStore(text)),
            Event::PtyWrite(text) => self.write(text),
            Event::TextAreaSizeRequest(format) => {
                let size = self.size.lock().map(|s| *s).ok();
                if let Some(size) = size {
                    self.write(format(size.window_size()));
                }
            }
            Event::ColorRequest(index, format) => {
                let c = match index {
                    0..=255 => self.palette.indexed(index as u8),
                    257 => self.palette.background,
                    258 => self.palette.cursor,
                    _ => self.palette.foreground,
                };
                self.write(format(AnsiRgb {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                }));
            }
            Event::ChildExit(status) => self.emit(TermEvent::Exited(status.code())),
            Event::Exit => self.emit(TermEvent::Exited(None)),
            Event::MouseCursorDirty | Event::CursorBlinkingChange | Event::ClipboardLoad(..) => {}
        }
    }
}

pub struct TerminalSession {
    term: Arc<FairMutex<Term<Proxy>>>,
    pty: EventLoopSender,
    size: Arc<Mutex<GridSize>>,
    palette: Arc<TerminalPalette>,
    child_pid: u32,
}

impl TerminalSession {
    pub fn spawn(
        options: SpawnOptions,
        size: GridSize,
        palette: TerminalPalette,
    ) -> anyhow::Result<(Self, async_channel::Receiver<TermEvent>)> {
        let (tx, rx) = async_channel::bounded(1024);
        let palette = Arc::new(palette);
        let shared_size = Arc::new(Mutex::new(size));
        let pty_slot = Arc::new(Mutex::new(None));
        let proxy = Proxy {
            events: tx,
            pty: pty_slot.clone(),
            size: shared_size.clone(),
            palette: palette.clone(),
        };

        let mut env = options.env;
        env.entry("TERM".into())
            .or_insert_with(|| "xterm-256color".into());
        env.entry("COLORTERM".into())
            .or_insert_with(|| "truecolor".into());
        let program = options.program.or_else(|| std::env::var("SHELL").ok());
        let tty_options = tty::Options {
            shell: program.map(|p| tty::Shell::new(p, options.args)),
            working_directory: options.cwd,
            drain_on_exit: true,
            env,
        };
        let pty = tty::new(&tty_options, size.window_size(), 0)?;
        let child_pid = pty.child().id();

        let config = Config {
            scrolling_history: options.scrollback.unwrap_or(10_000),
            ..Config::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, proxy.clone())));
        let event_loop = EventLoop::new(term.clone(), proxy, pty, true, false)?;
        let sender = event_loop.channel();
        if let Ok(mut slot) = pty_slot.lock() {
            *slot = Some(sender.clone());
        }
        event_loop.spawn();

        let session = Self {
            term,
            pty: sender,
            size: shared_size,
            palette,
            child_pid,
        };
        Ok((session, rx))
    }

    pub fn child_pid(&self) -> u32 {
        self.child_pid
    }

    pub fn size(&self) -> GridSize {
        self.size.lock().map(|s| *s).unwrap_or(GridSize {
            cols: 80,
            rows: 24,
            cell_width: 8,
            cell_height: 16,
        })
    }

    pub fn resize(&self, size: GridSize) {
        if let Ok(mut current) = self.size.lock() {
            if *current == size {
                return;
            }
            *current = size;
        }
        self.term.lock().resize(size);
        let _ = self.pty.send(Msg::Resize(size.window_size()));
    }

    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        self.term.lock().scroll_display(Scroll::Bottom);
        let _ = self.pty.send(Msg::Input(bytes.into()));
    }

    pub fn scroll(&self, lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(lines));
    }

    pub fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    pub fn snapshot(&self) -> Snapshot {
        snapshot(&self.term.lock(), &self.palette)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.pty.send(Msg::Shutdown);
    }
}
