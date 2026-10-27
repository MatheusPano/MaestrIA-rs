use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

const FILES: &[(&str, &[u8])] = &[
    ("icons/claude.svg", include_bytes!("../../assets/icons/claude.svg")),
    ("icons/terminal.svg", include_bytes!("../../assets/icons/terminal.svg")),
    ("glyphs/open_with.svg", include_bytes!("../../assets/glyphs/open_with.svg")),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}
