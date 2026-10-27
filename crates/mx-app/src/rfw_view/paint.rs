use std::f32::consts::{PI, TAU};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{App, Bounds, Hsla, PathBuilder, Pixels, Point, Styled, Window, canvas, point, prelude::*, px};
use mx_rfw::widgets::CircularProgressIndicator;

use super::layout::{Ctx, El};
use super::style::{argb, hsla};

fn at(c: Point<Pixels>, r: f32, angle: f32) -> Point<Pixels> {
    point(c.x + px(r * angle.cos()), c.y + px(r * angle.sin()))
}

fn arc(window: &mut Window, bounds: Bounds<Pixels>, stroke: f32, from: f32, sweep: f32, color: Hsla) {
    if sweep <= 0.0 {
        return;
    }
    let c = bounds.center();
    let side = f32::from(bounds.size.width.min(bounds.size.height));
    let r = ((side - stroke) / 2.0).max(0.5);
    let mut path = PathBuilder::stroke(px(stroke));
    path.move_to(at(c, r, from));
    let sweep = sweep.min(TAU - 0.0001);
    let mut done = 0.0;
    while done < sweep {
        let step = (sweep - done).min(PI * 0.9);
        done += step;
        path.arc_to(point(px(r), px(r)), px(0.0), false, true, at(c, r, from + done));
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

fn spin_phase() -> f32 {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    (ms % 1333) as f32 / 1333.0
}

pub fn ring(value: Option<f64>, stroke: f32, color: Hsla, track: Option<Hsla>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window: &mut Window, _: &mut App| {
            if let Some(track) = track {
                arc(window, bounds, stroke, -PI / 2.0, TAU, track);
            }
            match value {
                Some(v) => arc(window, bounds, stroke, -PI / 2.0, v.clamp(0.0, 1.0) as f32 * TAU, color),
                None => {
                    let start = spin_phase() * TAU - PI / 2.0;
                    arc(window, bounds, stroke, start, TAU * 0.28, color);
                    window.request_animation_frame();
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub fn circular(cx: &mut Ctx, p: &CircularProgressIndicator) -> El {
    let color = p.color.map_or(hsla(cx.palette.accent), argb);
    let track = p.background_color.map(argb);
    cx.el()
        .relative()
        .min_w(px(36.0))
        .min_h(px(36.0))
        .child(ring(p.value, p.stroke_width as f32, color, track))
}
