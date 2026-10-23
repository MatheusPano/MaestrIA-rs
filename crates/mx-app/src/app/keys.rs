use mx_store::Store;

use crate::keymap::{self, Action};

pub const SIDEBAR: &str = "Ctrl+Shift+B";
pub const SEARCH: &str = "Ctrl+Shift+F";
pub const SETTINGS: &str = "Ctrl+,";
pub const CLOSE_PANE: &str = "Ctrl+Shift+W";

pub fn slot(index: usize) -> Option<String> {
    (index < 9).then(|| mx_plugins::chord::Chord::slot(index as u8, keymap::LINUX).label(keymap::LINUX))
}

pub fn label(store: &Store, action: Action) -> Option<String> {
    keymap::keymap_of(store).first_label(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_nine_tabs_get_a_slot_label() {
        assert_eq!(slot(0).as_deref(), Some("Alt+1"));
        assert_eq!(slot(8).as_deref(), Some("Alt+9"));
        assert_eq!(slot(9), None);
    }

    #[test]
    fn fixed_labels_match_the_linux_defaults() {
        let m = keymap::Keymap::defaults(true);
        assert_eq!(m.first_label(Action::Sidebar).as_deref(), Some(SIDEBAR));
        assert_eq!(m.first_label(Action::Search).as_deref(), Some(SEARCH));
        assert_eq!(m.first_label(Action::Settings).as_deref(), Some(SETTINGS));
        assert_eq!(m.first_label(Action::Close).as_deref(), Some(CLOSE_PANE));
    }
}
