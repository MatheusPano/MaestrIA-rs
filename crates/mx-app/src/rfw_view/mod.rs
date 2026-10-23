#![allow(dead_code)]

mod blocks;
mod blocks_button;
mod blocks_fields;
mod blocks_list;
mod blocks_misc;
mod blocks_text;
mod demo;
mod drag;
mod icon_paths;
mod icons;
mod layout;
mod menu;
mod paint;
mod style;
mod text_input;
mod tooltip;
mod view;
mod widgets_core;
mod widgets_maestria;
mod widgets_scroll;

use std::rc::Rc;

use gpui::{App, Window};

#[allow(unused_imports)]
pub use blocks::BlocksView;
#[allow(unused_imports)]
pub use demo::{DEMO_ENV, open_demo};
#[allow(unused_imports)]
pub use style::rfw_theme;
#[allow(unused_imports)]
pub use view::RfwView;

#[derive(Debug, Clone, PartialEq)]
pub struct ViewAction {
    pub action: String,
    pub values: serde_json::Map<String, serde_json::Value>,
}

pub type ActionSink = Rc<dyn Fn(ViewAction, &mut Window, &mut App)>;

#[cfg(test)]
mod tests;
