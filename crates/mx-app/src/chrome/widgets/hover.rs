use gpui::{Hsla, InteractiveElement, Styled, px};

pub const HOVER_MARGIN: f32 = 7.;
pub const HOVER_RADIUS: f32 = 9.;

pub trait MxHover: Styled + InteractiveElement + Sized {
    fn inset_hover(self, h: f32, v: f32, radius: f32, color: Option<Hsla>) -> Self {
        let el = self.mx(px(h)).rounded(px(radius));
        let el = if v > 0. { el.my(px(v)) } else { el };
        match color {
            Some(c) => el.hover(move |s| s.bg(c)),
            None => el,
        }
    }
}

impl<E: Styled + InteractiveElement> MxHover for E {}
