use std::f32::consts::PI;

use crate::chrome::anim::confetti::{Rng, Variant, pose, spawn};
use crate::chrome::anim::ease::interval;
use crate::chrome::anim::reveal::reveal_offset;
use crate::chrome::notices::toasts_frames;
use crate::chrome::settings::{hibernate_label, hibernate_less, hibernate_more, usage_tone};
use crate::chrome::menu::Tone;

#[test]
fn fire_shoots_55_per_cannon_from_both_bottom_corners_upward() {
    let ps = spawn(Variant::Fire, (0., 0.), &mut Rng::new(7));
    assert_eq!(ps.len(), 110);
    let left: Vec<_> = ps.iter().filter(|p| p.ox == 0.).collect();
    assert_eq!(left.len(), 55);
    for p in &ps {
        assert_eq!(p.oy, 1.);
        let up = if p.ox == 0. { p.angle } else { PI - p.angle };
        assert!((0.19 * PI..=0.44 * PI).contains(&up));
        assert!((2.1..=3.1).contains(&p.speed));
        assert!((0. ..=0.16).contains(&p.delay));
        assert!(p.color < 7);
    }
}

#[test]
fn puff_shoots_40_from_the_click_without_delay() {
    let ps = spawn(Variant::Puff, (0.25, 0.5), &mut Rng::new(3));
    assert_eq!(ps.len(), 40);
    assert!(ps.iter().all(|p| p.ox == 0.25 && p.oy == 0.5 && p.delay == 0. && (0.6..=1.5).contains(&p.speed)));
}

#[test]
fn particles_follow_the_closed_form_and_fade_late() {
    let p = spawn(Variant::Puff, (0.5, 0.5), &mut Rng::new(11))[0];
    let a = pose(&p, Variant::Puff, 0.1, 800., 600.).unwrap();
    let k = 1.15f32;
    let decay = (1. - (-k * 0.1f32).exp()) / k;
    let vt = 1.5 * 600. / k;
    assert!((a.x - (400. + p.angle.cos() * p.speed * 600. * decay)).abs() < 0.01);
    assert!((a.y - (300. + (-p.angle.sin() * p.speed * 600. - vt) * decay + vt * 0.1)).abs() < 0.01);
    assert_eq!(a.alpha, 1.);
    let mut up = p;
    up.angle = PI / 2.;
    up.speed = 1.5;
    let late = pose(&up, Variant::Puff, 1.3, 800., 600.).unwrap();
    assert!(late.alpha < 1. && late.alpha > 0.);
    let mut delayed = p;
    delayed.delay = 0.1;
    assert!(pose(&delayed, Variant::Puff, 0.05, 800., 600.).is_none());
}

#[test]
fn particles_below_the_overlay_are_skipped() {
    let mut p = spawn(Variant::Puff, (0.5, 1.), &mut Rng::new(5))[0];
    p.angle = -PI / 2.;
    assert!(pose(&p, Variant::Puff, 1.0, 100., 100.).is_none());
}

#[test]
fn reveal_scrolls_row_to_thirty_percent() {
    assert_eq!(reveal_offset(1000., 40., 300., 3000.), 1000. - 0.3 * 260.);
    assert_eq!(reveal_offset(10., 40., 300., 3000.), 0.);
    assert_eq!(reveal_offset(2990., 40., 300., 3000.), 2700.);
}

#[test]
fn toast_enters_with_size_first_then_slide_and_fade() {
    let (size, dx, fade) = toasts_frames::enter_frame(0.);
    assert_eq!((size, fade), (0., 0.));
    assert!((dx - 1.15 * 380.).abs() < 0.01);
    let (size, _, fade) = toasts_frames::enter_frame(0.45);
    assert_eq!(size, 1.);
    assert!(fade > 0. && fade < 1.);
    assert_eq!(toasts_frames::enter_frame(1.), (1., 0., 1.));
    assert_eq!(interval(0.1, 0.25, 1.), 0.);
}

#[test]
fn hibernation_stepper_moves_in_coarser_steps_and_labels_hours() {
    assert_eq!(hibernate_less(5), 5);
    assert_eq!(hibernate_less(30), 25);
    assert_eq!(hibernate_less(120), 105);
    assert_eq!(hibernate_less(180), 120);
    assert_eq!(hibernate_more(25), 30);
    assert_eq!(hibernate_more(30), 45);
    assert_eq!(hibernate_more(120), 180);
    assert_eq!(hibernate_more(1440), 1440);
    assert_eq!(hibernate_label(45), "45 min");
    assert_eq!(hibernate_label(120), "2 h");
    assert_eq!(hibernate_label(90), "90 min");
}

#[test]
fn usage_meter_turns_yellow_at_75_and_red_at_90() {
    assert_eq!(usage_tone(74.), Tone::Accent);
    assert_eq!(usage_tone(75.), Tone::Yellow);
    assert_eq!(usage_tone(90.), Tone::Red);
}
