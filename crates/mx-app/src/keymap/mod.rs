mod action;
mod chord;
mod dispatch;
mod map;
mod settings;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_store;

pub use action::Action;
pub use chord::{
    LINUX, TerminalKey, check_chord, copy_chord, paste_chords, press_of,
    select_all_chord, terminal_key,
};
pub use dispatch::{Outcome, UiRequest, keymap_of, on_key_down};
pub use settings::{apply, shortcut_rows, zoom_keys};
#[cfg(test)]
pub use map::Keymap;
#[cfg(test)]
use chord::check_chord_for;
