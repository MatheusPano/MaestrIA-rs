use std::f32::consts::PI;
use std::sync::LazyLock;
use std::time::Instant;

use gpui::{
    App, Bounds, IntoElement, PathBuilder, Pixels, RenderOnce, Styled, Window, canvas, point, px,
};
use mx_theme::Color;

use super::ease::ease_in_out;
use crate::chrome::style::alpha;

static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

pub const SPIN_MS: f32 = 950.;
pub const PULSE_MS: f32 = 1100.;

fn phase(period_ms: f32) -> f32 {
    (EPOCH.elapsed().as_secs_f32() * 1000. % period_ms) / period_ms
}

fn ring(window: &mut Window, center: gpui::Point<Pixels>, r: f32, from: f32, sweep: f32, width: f32, color: gpui::Hsla) {
    let steps = ((sweep.abs() / (2. * PI)) * 48.).ceil().max(4.) as usize;
    let mut path = PathBuilder::stroke(px(width));
    for i in 0..=steps {
        let a = from + sweep * i as f32 / steps as f32;
        let p = point(center.x + px(r * a.cos()), center.y + px(r * a.sin()));
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

fn center_of(b: Bounds<Pixels>) -> gpui::Point<Pixels> {
    b.center()
}

#[derive(IntoElement)]
pub struct Spinner {
    size: f32,
    color: Color,
}

impl Spinner {
    pub fn new(size: f32, color: Color) -> Self {
        Self { size, color }
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let color = self.color;
        canvas(|_, _, _| {}, move |b, _, window, _| {
            paint_spin(window, b, color, 1.);
            window.request_animation_frame();
        })
        .size(px(self.size))
        .flex_none()
    }
}

pub(super) fn paint_spin(window: &mut Window, b: Bounds<Pixels>, color: Color, opacity: f32) {
    let r = f32::from(b.size.width.min(b.size.height)) / 2. - 1.;
    let c = center_of(b);
    ring(window, c, r, 0., 2. * PI, 1.1, alpha(color, 0.22 * opacity as f64));
    let start = phase(SPIN_MS) * 2. * PI - PI / 2.;
    ring(window, c, r, start, 1.05 * PI, 1.3, alpha(color, opacity as f64));
}

pub fn pulse_t(phase: f32) -> f32 {
    let tri = if phase < 0.5 { phase * 2. } else { 2. - phase * 2. };
    ease_in_out(tri)
}

#[derive(IntoElement)]
pub struct Pulse {
    size: f32,
    color: Color,
}

impl Pulse {
    pub fn new(size: f32, color: Color) -> Self {
        Self { size, color }
    }
}

impl RenderOnce for Pulse {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let color = self.color;
        canvas(|_, _, _| {}, move |b, _, window, _| {
            paint_pulse(window, b, color, 1.);
            window.request_animation_frame();
        })
        .size(px(self.size))
        .flex_none()
    }
}

pub(super) fn paint_pulse(window: &mut Window, b: Bounds<Pixels>, color: Color, opacity: f32) {
    let t = pulse_t((EPOCH.elapsed().as_secs_f32() * 1000. % (2. * PULSE_MS)) / (2. * PULSE_MS));
    let r = f32::from(b.size.width.min(b.size.height)) / 2. - 1.;
    let c = center_of(b);
    ring(window, c, r, 0., 2. * PI, 1.1, alpha(color, ((0.20 + 0.30 * t) * opacity) as f64));
    let dot = r * (0.26 + 0.16 * t);
    let bounds = Bounds::centered_at(c, gpui::size(px(dot * 2.), px(dot * 2.)));
    window.paint_quad(gpui::fill(bounds, alpha(color, ((0.55 + 0.45 * t) * opacity) as f64)).corner_radii(px(dot)));
}
