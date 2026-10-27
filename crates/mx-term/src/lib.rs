#![forbid(unsafe_code)]

mod events;
mod follow;
mod ime;
mod io;
mod keys;
mod links;
mod mouse;
mod paste;
mod port;
mod scrubber;
mod session;
mod signals;
mod snapshot;
mod spawn;
mod text;

pub use alacritty_terminal::term::TermMode;
pub use events::{DirtyFlag, Subscriber};
pub use follow::{LinkFollow, follow_link, url_failed_banner};
pub use ime::{TextInputGuard, TextOrigin};
pub use keys::{Mods, encode_key, is_slot_chord, is_text_key};
pub use links::{link_at, local_path, resolve_link_path, trim_tail};
pub use mouse::{MouseAction, MouseButton, WheelAccumulator, clamp_cell, reports_mouse, wheel_bytes};
pub use port::{TabExitHook, TermRegistry};
pub use scrubber::VtScrubber;
pub use session::{
    ExitHook, GridSize, PasteOutcome, SCROLLBACK, SelectionKind, SpawnOptions, TermEvent,
    TerminalSession,
};
pub use signals::GRACE;
pub use snapshot::{CellStyle, CursorKind, Marks, SnapCell, SnapCursor, Snapshot};
pub use spawn::{Launch, SpawnPlan, TERM};
mod session_api;
mod trust;
pub use session_api::{KEY_GAP, key_text};
pub use trust::asks_trust;
mod rows;
mod reflow;
mod find;
mod find_session;
mod sticky;
pub use find::{FIND_LIMIT, Hit, find_in_rows};
pub use find_session::{FindSession, Viewport};
pub use rows::{CellPos, Row, RowCell};
