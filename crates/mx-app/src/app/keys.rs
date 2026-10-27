use mx_store::Store;

use crate::keymap::{self, Action};

pub const SIDEBAR: &str = "Ctrl+Shift+B";
pub const CLOSE_PANE: &str = "Ctrl+Shift+W";

pub fn label(store: &Store, action: Action) -> Option<String> {
    keymap::keymap_of(store).first_label(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_labels_match_the_linux_defaults() {
        let m = keymap::Keymap::defaults(true);
        assert_eq!(m.first_label(Action::Sidebar).as_deref(), Some(SIDEBAR));
        assert_eq!(m.first_label(Action::Search).as_deref(), Some("Ctrl+Alt+F"));
        assert_eq!(m.first_label(Action::FindInPane).as_deref(), Some("Ctrl+Shift+F"));
        assert_eq!(m.first_label(Action::Settings).as_deref(), Some("Ctrl+,"));
        assert_eq!(m.first_label(Action::Close).as_deref(), Some(CLOSE_PANE));
    }
}
