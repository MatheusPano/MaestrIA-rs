use std::f32::consts::PI;

use gpui::{
    App, BorderStyle, Bounds, Hsla, IntoElement, Negate, Pixels, Point, RenderOnce, SharedString, Styled,
    TransformationMatrix, Window, canvas, px, quad, radians, size, transparent_black,
};
use mx_theme::Color;

use super::clock::now_ms;
use super::ease::ease_in_out;
use crate::chrome::style::alpha;

pub const SPIN_MS: f32 = 1200.;
pub const PULSE_MS: f32 = 1100.;
const ARC_SVG: &str = "glyphs/spin_arc.svg";

fn phase(at_ms: u64, period_ms: f32) -> f32 {
    (at_ms % period_ms as u64) as f32 / period_ms
}

fn ring(window: &mut Window, center: Point<Pixels>, r: f32, width: f32, color: Hsla) {
    let side = 2. * r + width;
    let bounds = Bounds::centered_at(center, size(px(side), px(side)));
    window.paint_quad(quad(bounds, px(side / 2.), transparent_black(), px(width), color, BorderStyle::Solid));
}

fn arc(window: &mut Window, b: Bounds<Pixels>, turn: f32, color: Hsla, cx: &App) {
    let scale = window.scale_factor();
    let center = b.center().scale(scale);
    let spin = TransformationMatrix::unit()
        .translate(center)
        .rotate(radians(turn * 2. * PI))
        .translate(center.negate());
    let _ = window.paint_svg(b, SharedString::new_static(ARC_SVG), spin, color, cx);
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
        canvas(|_, _, _| {}, move |b, _, window, cx| {
            paint_spin(window, b, color, 1., now_ms(), cx);
            window.request_animation_frame();
        })
        .size(px(self.size))
        .flex_none()
    }
}

pub(super) fn paint_spin(window: &mut Window, b: Bounds<Pixels>, color: Color, opacity: f32, at_ms: u64, cx: &App) {
    let r = f32::from(b.size.width.min(b.size.height)) / 2. - 1.;
    ring(window, center_of(b), r, 1.1, alpha(color, 0.22 * opacity as f64));
    arc(window, b, phase(at_ms, SPIN_MS) - 0.25, alpha(color, opacity as f64), cx);
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
            paint_pulse(window, b, color, 1., now_ms());
            window.request_animation_frame();
        })
        .size(px(self.size))
        .flex_none()
    }
}

pub(super) fn paint_pulse(window: &mut Window, b: Bounds<Pixels>, color: Color, opacity: f32, at_ms: u64) {
    let t = pulse_t(phase(at_ms, 2. * PULSE_MS));
    let r = f32::from(b.size.width.min(b.size.height)) / 2. - 1.;
    let c = center_of(b);
    ring(window, c, r, 1.1, alpha(color, ((0.20 + 0.30 * t) * opacity) as f64));
    let dot = r * (0.26 + 0.16 * t);
    let bounds = Bounds::centered_at(c, gpui::size(px(dot * 2.), px(dot * 2.)));
    window.paint_quad(gpui::fill(bounds, alpha(color, ((0.55 + 0.45 * t) * opacity) as f64)).corner_radii(px(dot)));
}
