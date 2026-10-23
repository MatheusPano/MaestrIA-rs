use std::time::{Duration, Instant};

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, PathBuilder, Pixels, Point, Render, Styled, Window,
    canvas, div, point, px,
};
use mx_theme::{Color, Theme};

use super::confetti::{Particle, Rng, Variant, pose, spawn};
use crate::chrome::style::{hsla, theme};

struct Burst {
    id: u64,
    variant: Variant,
    start: Instant,
    particles: Vec<Particle>,
}

#[derive(Default)]
pub struct ConfettiLayer {
    bursts: Vec<Burst>,
    next: u64,
}

pub fn colors(t: &Theme) -> [Color; 7] {
    [t.accent, t.green, t.yellow, t.purple, t.red, t.ansi.cyan, t.ansi.blue]
}

impl ConfettiLayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> bool {
        !self.bursts.is_empty()
    }

    pub fn fire(&mut self, cx: &mut Context<Self>) {
        self.launch(Variant::Fire, (0., 1.), cx);
    }

    pub fn puff(&mut self, at: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        let size = window.viewport_size();
        let fx = f32::from(at.x) / f32::from(size.width).max(1.);
        let fy = f32::from(at.y) / f32::from(size.height).max(1.);
        self.launch(Variant::Puff, (fx, fy), cx);
    }

    fn launch(&mut self, variant: Variant, origin: (f32, f32), cx: &mut Context<Self>) {
        self.next += 1;
        let id = self.next;
        let seed = Instant::now().elapsed().as_nanos() as u64 ^ (id.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        self.bursts.push(Burst {
            id,
            variant,
            start: Instant::now(),
            particles: spawn(variant, origin, &mut Rng::new(seed | 1)),
        });
        let life = Duration::from_millis(variant.life_ms());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(life).await;
            this.update(cx, |this, cx| {
                this.bursts.retain(|b| b.id != id);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

impl Render for ConfettiLayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.bursts.is_empty() {
            return div().into_any_element();
        }
        let palette = colors(&theme(cx));
        let frames: Vec<(Variant, f32, Vec<Particle>)> = self
            .bursts
            .iter()
            .map(|b| (b.variant, b.start.elapsed().as_secs_f32(), b.particles.clone()))
            .collect();
        div()
            .absolute()
            .inset_0()
            .debug_selector(|| "confetti".into())
            .child(
                canvas(|_, _, _| {}, move |bounds, _, window, _| {
                    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    for (variant, elapsed, particles) in &frames {
                        for p in particles {
                            let Some(pose) = pose(p, *variant, *elapsed, w, h) else {
                                continue;
                            };
                            let (s, c) = pose.rotation.sin_cos();
                            let hw = p.w / 2. * pose.scale_x;
                            let hh = p.h / 2.;
                            let corner = |dx: f32, dy: f32| {
                                point(
                                    bounds.origin.x + px(pose.x + dx * c - dy * s),
                                    bounds.origin.y + px(pose.y + dx * s + dy * c),
                                )
                            };
                            let mut path = PathBuilder::fill();
                            path.move_to(corner(-hw, -hh));
                            path.line_to(corner(hw, -hh));
                            path.line_to(corner(hw, hh));
                            path.line_to(corner(-hw, hh));
                            path.close();
                            if let Ok(path) = path.build() {
                                let color = palette[p.color].with_alpha(pose.alpha as f64);
                                window.paint_path(path, hsla(color));
                            }
                        }
                    }
                    window.request_animation_frame();
                })
                .size_full(),
            )
            .into_any_element()
    }
}

