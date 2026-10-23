use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

pub const CLAUDE_SVG: &str = "icons/claude.svg";
pub const TERMINAL_SVG: &str = "icons/terminal.svg";

const FILES: [(&str, &[u8]); 2] = [
    (CLAUDE_SVG, include_bytes!("assets/claude.svg")),
    (TERMINAL_SVG, include_bytes!("assets/terminal.svg")),
];

pub struct ChromeAssets;

impl ChromeAssets {
    pub fn get(path: &str) -> Option<&'static [u8]> {
        FILES.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
    }
}

impl AssetSource for ChromeAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}
