use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::Duration;

use gpui::{
    App, Bounds, ContentMask, EntityId, Global, IntoElement, Pixels, RenderOnce, Styled, Window, canvas, px,
};
use mx_theme::Color;

use super::clock::{now_ms, until_next};
use super::spinner::{paint_pulse, paint_spin};
use crate::chrome::notices::anim_off;

pub const STEP_MS: u64 = 150;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Spin,
    Pulse,
}

#[derive(Clone, Debug)]
struct Slot {
    bounds: Bounds<Pixels>,
    mask: ContentMask<Pixels>,
    motion: Motion,
    color: Color,
    opacity: f32,
}

#[derive(Default)]
pub struct Slots {
    by_owner: RefCell<HashMap<EntityId, Vec<Slot>>>,
    pending: Cell<bool>,
}

impl Global for Slots {}

pub fn forget(owner: EntityId, cx: &mut App) {
    cx.default_global::<Slots>().by_owner.borrow_mut().remove(&owner);
}

#[derive(IntoElement)]
pub struct Animated {
    size: f32,
    motion: Motion,
    color: Color,
    opacity: f32,
}

impl Animated {
    pub fn new(motion: Motion, size: f32, color: Color) -> Self {
        Self { size, motion, color, opacity: 1. }
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}

impl RenderOnce for Animated {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let Animated { motion, color, opacity, .. } = self;
        canvas(
            move |bounds, window, cx| {
                let slot = Slot { bounds, mask: window.content_mask(), motion, color, opacity };
                let owner = window.current_view();
                cx.default_global::<Slots>().by_owner.borrow_mut().entry(owner).or_default().push(slot);
            },
            |_, _, _, _| {},
        )
        .size(px(self.size))
        .flex_none()
    }
}

pub fn layer(owners: Vec<EntityId>) -> impl IntoElement {
    canvas(|_, _, _| {}, move |_, _, window, cx| paint_layer(&owners, window, cx)).absolute().size_0()
}

fn paint_layer(owners: &[EntityId], window: &mut Window, cx: &mut App) {
    let slots: Vec<Slot> = {
        let global = cx.default_global::<Slots>();
        let map = global.by_owner.borrow();
        owners.iter().filter_map(|o| map.get(o)).flatten().filter(|s| on_screen(s)).cloned().collect()
    };
    if slots.is_empty() {
        return;
    }
    let now = now_ms();
    let at = now - now % STEP_MS;
    for s in &slots {
        window.with_content_mask(Some(s.mask.clone()), |window| match s.motion {
            Motion::Spin => paint_spin(window, s.bounds, s.color, s.opacity, at, cx),
            Motion::Pulse => paint_pulse(window, s.bounds, s.color, s.opacity, at),
        });
    }
    if anim_off() || !window.is_window_active() || cx.global::<Slots>().pending.replace(true) {
        return;
    }
    let view = window.current_view();
    let wait = Duration::from_millis(until_next(STEP_MS, now));
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(wait).await;
            cx.update(|_, cx| {
                cx.global::<Slots>().pending.set(false);
                cx.notify(view);
            })
            .ok();
        })
        .detach();
}

fn on_screen(s: &Slot) -> bool {
    !s.bounds.intersect(&s.mask.bounds).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_clipped_out_of_its_scroll_view_is_not_on_screen() {
        let slot = |y: f32| Slot {
            bounds: Bounds::new(gpui::point(px(0.), px(y)), gpui::size(px(10.), px(10.))),
            mask: ContentMask { bounds: Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(100.), px(100.))) },
            motion: Motion::Spin,
            color: Color::WHITE,
            opacity: 1.,
        };
        assert!(on_screen(&slot(50.)));
        assert!(on_screen(&slot(95.)));
        assert!(!on_screen(&slot(140.)));
    }
}
