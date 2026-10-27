use crate::chrome::icon;
use crate::chrome::menu::{MenuEntry, MenuItem, Tone};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeMenu {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

pub fn menu_entries(selected: bool, mac: bool) -> Vec<MenuEntry<CodeMenu>> {
    let meta = if mac { "⌘" } else { "Ctrl+" };
    vec![
        MenuItem::new("Recortar", CodeMenu::Cut).chord(Some(format!("{meta}X"))).disabled(!selected).into(),
        MenuItem::new("Copiar", CodeMenu::Copy)
            .icon(icon::CONTENT_COPY, Tone::Dim)
            .chord(Some(format!("{meta}C")))
            .disabled(!selected)
            .into(),
        MenuItem::new("Colar", CodeMenu::Paste).icon(icon::CONTENT_PASTE, Tone::Dim).chord(Some(format!("{meta}V"))).into(),
        MenuEntry::Divider,
        MenuItem::new("Selecionar tudo", CodeMenu::SelectAll)
            .icon(icon::DONE_ALL_ROUNDED, Tone::Dim)
            .chord(Some(format!("{meta}A")))
            .into(),
    ]
}
