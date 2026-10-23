#![allow(dead_code, unused_imports)]

pub mod anim;
pub mod assets;
pub mod demo;
pub mod dialog;
pub mod flow;
pub mod icon;
mod icon_table;
pub mod menu;
pub mod notices;
pub mod settings;
pub mod style;
pub mod widgets;

use std::borrow::Cow;

use gpui::App;

const ICON_FONT: &[u8] = include_bytes!("assets/material-icons.otf");

pub fn register_fonts(cx: &mut App) {
    if let Err(err) = cx.text_system().add_fonts(vec![Cow::Borrowed(ICON_FONT)]) {
        log::error!("failed to load the icon font: {err}");
    }
}

#[cfg(test)]
mod tests;
