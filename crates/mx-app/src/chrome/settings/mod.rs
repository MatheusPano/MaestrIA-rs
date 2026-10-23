mod account;
mod appearance;
mod dialog;
mod gallery;
pub mod model;
mod plugins;
mod programs;
mod shortcuts;

pub use dialog::{Recording, SettingsDialog, Validate};
pub use model::*;
pub use plugins::{plugin_menu, state_tone};
pub use shortcuts::{GROUPS, RECORD_PROMPT, fixed_rows};
