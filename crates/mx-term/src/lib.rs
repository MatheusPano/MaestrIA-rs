#![forbid(unsafe_code)]

mod keys;
mod session;
mod snapshot;

pub use alacritty_terminal::term::TermMode;
pub use keys::{Mods, encode_key};
pub use session::{GridSize, SpawnOptions, TermEvent, TerminalSession};
pub use snapshot::{CellStyle, CursorKind, SnapCell, SnapCursor, Snapshot};
