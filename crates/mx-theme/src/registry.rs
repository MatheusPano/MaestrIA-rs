use std::sync::Arc;

use crate::builtin::{built_in, built_ins, default_theme, is_built_in, retired_alias};
use crate::{Palette, Theme, ThemeError};

#[derive(Clone, Debug, Default)]
pub struct Themes {
    extra: Vec<Arc<Theme>>,
}

impl Themes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn built_ins(&self) -> &'static [Arc<Theme>] {
        built_ins()
    }

    pub fn extra(&self) -> &[Arc<Theme>] {
        &self.extra
    }

    pub fn by_id(&self, id: Option<&str>) -> Arc<Theme> {
        let Some(id) = id else { return default_theme() };
        self.find(id)
            .or_else(|| retired_alias(id).and_then(|to| self.find(to)))
            .unwrap_or_else(default_theme)
    }

    fn find(&self, id: &str) -> Option<Arc<Theme>> {
        built_in(id)
            .cloned()
            .or_else(|| self.extra.iter().find(|t| t.id == id).cloned())
    }

    pub fn register(&mut self, palette: Palette) -> Result<Arc<Theme>, ThemeError> {
        if is_built_in(&palette.id) {
            return Err(ThemeError::BuiltInId(palette.id));
        }
        if self.extra.iter().any(|t| t.id == palette.id) {
            return Err(ThemeError::DuplicateId(palette.id));
        }
        let theme = Arc::new(Theme::new(palette));
        self.extra.push(theme.clone());
        Ok(theme)
    }

    pub fn clear_extra(&mut self) {
        self.extra.clear();
    }
}
