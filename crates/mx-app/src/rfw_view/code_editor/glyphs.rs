use gpui::{AnyElement, Hsla, IntoElement, ParentElement, Styled, div, img, px};

use crate::rfw_view::icons::raw_svg;
use crate::rfw_view::style::{hex, opacity_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    ChevronRight,
    ExpandMore,
    ArrowUp,
    ArrowDown,
    Close,
    FindReplace,
    DoneAll,
}

fn path(g: Glyph) -> &'static str {
    match g {
        Glyph::ChevronRight => "M9.29 6.71a1 1 0 0 0 0 1.41L13.17 12l-3.88 3.88a1 1 0 1 0 1.41 1.41l4.59-4.59a1 1 0 0 0 0-1.41L10.7 6.7a1 1 0 0 0-1.41.01z",
        Glyph::ExpandMore => "M15.88 9.29 12 13.17 8.12 9.29a1 1 0 1 0-1.41 1.41l4.59 4.59a1 1 0 0 0 1.41 0l4.59-4.59a1 1 0 0 0 0-1.41 1.02 1.02 0 0 0-1.42 0z",
        Glyph::ArrowUp => "M13 19V7.83l4.88 4.88a1 1 0 0 0 1.42 0 1 1 0 0 0 0-1.41l-6.59-6.59a1 1 0 0 0-1.41 0l-6.6 6.58a1 1 0 1 0 1.41 1.41L11 7.83V19a1 1 0 0 0 2 0z",
        Glyph::ArrowDown => "M11 5v11.17l-4.88-4.88a1 1 0 0 0-1.42 0 1 1 0 0 0 0 1.41l6.59 6.59a1 1 0 0 0 1.41 0l6.59-6.59a1 1 0 1 0-1.41-1.41L13 16.17V5a1 1 0 0 0-2 0z",
        Glyph::Close => "M18.3 5.71a1 1 0 0 0-1.41 0L12 10.59 7.11 5.7A1 1 0 0 0 5.7 7.11L10.59 12 5.7 16.89a1 1 0 1 0 1.41 1.41L12 13.41l4.89 4.89a1 1 0 0 0 1.41-1.41L13.41 12l4.89-4.89a1 1 0 0 0 0-1.4z",
        Glyph::FindReplace => "M11 6c1.38 0 2.63.56 3.54 1.46L12 10h6V4l-2.05 2.05A6.98 6.98 0 0 0 11 4c-3.53 0-6.43 2.61-6.92 6H6.1A5 5 0 0 1 11 6zm5.64 9.14A6.95 6.95 0 0 0 17.92 12H15.9A5 5 0 0 1 11 16c-1.38 0-2.63-.56-3.54-1.46L10 12H4v6l2.05-2.05A6.98 6.98 0 0 0 11 18c1.55 0 2.98-.51 4.14-1.36L20 21.49 21.49 20l-4.85-4.86z",
        Glyph::DoneAll => "M17.3 6.3a1 1 0 0 0-1.41 0l-5.64 5.64 1.41 1.41L17.3 7.7a1 1 0 0 0 0-1.4zm4.24-.01-9.88 9.88-3.48-3.47a1 1 0 1 0-1.41 1.41l4.18 4.18a1 1 0 0 0 1.41 0L22.95 7.71a1 1 0 0 0-1.41-1.42zM1.12 14.12 5.3 18.3a1 1 0 0 0 1.41 0l.7-.7-4.88-4.89a1 1 0 0 0-1.41 1.41z",
    }
}

pub fn glyph(g: Glyph, size: f32, color: Hsla) -> AnyElement {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 24 24\">\
         <path fill=\"{}\" fill-opacity=\"{}\" d=\"{}\"/></svg>",
        hex(color),
        opacity_of(color),
        path(g)
    );
    let frame = div().flex_none().size(px(size));
    match raw_svg(&svg) {
        Some(image) => frame.child(img(image).size_full()).into_any_element(),
        None => frame.into_any_element(),
    }
}
