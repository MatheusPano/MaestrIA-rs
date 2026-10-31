#![allow(dead_code)]

mod blocks;
mod blocks_button;
mod blocks_fields;
mod blocks_list;
mod blocks_misc;
mod blocks_text;
mod code_editor;
mod code_pane;
mod demo;
mod drag;
mod grid;
mod icon_paths;
mod icons;
mod layout;
mod menu;
mod paint;
mod picture;
mod style;
mod text_input;
mod tooltip;
mod view;
mod widgets_core;
mod widgets_maestria;
mod widgets_scroll;
mod anchor;
mod markdown_widget;
mod selectable;
mod sprite;
mod states;
mod text_input_widget;
mod zoom_view;

use std::rc::Rc;

use gpui::{App, Window};

#[allow(unused_imports)]
pub use blocks::{BlocksMode, BlocksView};
#[allow(unused_imports)]
pub use demo::{DEMO_ENV, open_demo};
#[allow(unused_imports)]
pub use style::rfw_theme;
#[allow(unused_imports)]
pub use view::RfwView;
pub use icons::named as named_icon;
pub use code_editor::own_code_buffers;
pub use anchor::anchor_rect;
pub use states::FollowLink;

pub fn plugin_glyph(icon: Option<&str>, dir: Option<&std::path::Path>, size: f32, color: gpui::Hsla) -> gpui::AnyElement {
    icons::glyph(icon, dir, size, color, 0.0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewAction {
    pub action: String,
    pub values: serde_json::Map<String, serde_json::Value>,
}

pub type ActionSink = Rc<dyn Fn(ViewAction, &mut Window, &mut App)>;

#[cfg(test)]
mod code_pane_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod widgets_tests;
