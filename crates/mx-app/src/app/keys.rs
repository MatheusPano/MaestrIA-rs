pub const SIDEBAR: &str = "Ctrl+Shift+B";
pub const SEARCH: &str = "Ctrl+Shift+F";
pub const SETTINGS: &str = "Ctrl+,";
pub const CLOSE_PANE: &str = "Ctrl+Shift+W";

pub fn slot(index: usize) -> Option<String> {
    (index < 9).then(|| format!("Alt+{}", index + 1))
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
}
