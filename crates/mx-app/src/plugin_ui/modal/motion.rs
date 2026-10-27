use std::time::{Duration, Instant};

use gpui::{Bounds, Pixels, point, px, size};
use mx_rfw::widgets::zoom_view::{ease_in_cubic, ease_out_cubic};

pub const OPEN: Duration = Duration::from_millis(340);
pub const CLOSE: Duration = Duration::from_millis(260);
pub const BACKDROP: f32 = 0.42;

pub fn card_size(viewport: (f32, f32), width: Option<f64>, height: Option<f64>) -> (f32, f32) {
    let (w, h) = viewport;
    let wide = (w - 48.).clamp(320., 820.);
    let width = (width.map_or(w - 48., |v| v as f32)).min(wide).max(320.);
    let height = (height.map_or(h - 48., |v| v as f32)).min(h * 0.88).max(240.);
    (width, height)
}

pub fn end_rect(viewport: (f32, f32), card: (f32, f32)) -> Bounds<Pixels> {
    let (cx, cy) = (viewport.0 / 2., viewport.1 / 2.);
    Bounds::new(point(px(cx - card.0 / 2.), px(cy - card.1 / 2.)), size(px(card.0), px(card.1)))
}

fn centre(b: &Bounds<Pixels>) -> (f32, f32) {
    (f32::from(b.origin.x + b.size.width / 2.), f32::from(b.origin.y + b.size.height / 2.))
}

pub struct Frame {
    pub card: Bounds<Pixels>,
    pub opacity: f32,
    pub backdrop: f32,
}

pub fn frame(from: Option<Bounds<Pixels>>, end: Bounds<Pixels>, t: f32) -> Frame {
    let (ew, eh) = (f32::from(end.size.width), f32::from(end.size.height));
    let start = from.unwrap_or_else(|| {
        let (cx, cy) = centre(&end);
        Bounds::new(point(px(cx - ew * 0.485), px(cy - eh * 0.485)), size(px(ew * 0.97), px(eh * 0.97)))
    });
    let ratio = f32::from(start.size.width) / ew.max(1.);
    let scale = (ratio + (1. - ratio) * t).clamp(0.02, 1.);
    let (sx, sy) = centre(&start);
    let (ex, ey) = centre(&end);
    let (cx, cy) = (sx + (ex - sx) * t, sy + (ey - sy) * t);
    let (w, h) = (ew * scale, eh * scale);
    Frame {
        card: Bounds::new(point(px(cx - w / 2.), px(cy - h / 2.)), size(px(w), px(h))),
        opacity: (t * 1.6).clamp(0., 1.),
        backdrop: BACKDROP * t,
    }
}

pub fn on_screen(rect: Bounds<Pixels>, viewport: (f32, f32)) -> bool {
    let (x, y) = (f32::from(rect.origin.x), f32::from(rect.origin.y));
    let (w, h) = (f32::from(rect.size.width), f32::from(rect.size.height));
    w > 0. && h > 0. && x + w > 0. && y + h > 0. && x < viewport.0 && y < viewport.1
}

#[derive(Debug, Clone, PartialEq)]
pub struct Motion {
    pub tab: String,
    pub from: Option<Bounds<Pixels>>,
    pub closing: bool,
    since: Instant,
    at: f32,
}

impl Motion {
    pub fn open(tab: &str, from: Option<Bounds<Pixels>>, now: Instant) -> Self {
        Self { tab: tab.to_owned(), from, closing: false, since: now, at: 0. }
    }

    pub fn value(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.since).as_secs_f32();
        if self.closing {
            (self.at - elapsed / CLOSE.as_secs_f32()).max(0.)
        } else {
            (self.at + elapsed / OPEN.as_secs_f32()).min(1.)
        }
    }

    pub fn t(&self, now: Instant) -> f32 {
        let v = self.value(now);
        if self.closing { ease_in_cubic(f64::from(v)) as f32 } else { ease_out_cubic(f64::from(v)) as f32 }
    }

    pub fn moving(&self, now: Instant) -> bool {
        let v = self.value(now);
        if self.closing { v > 0. } else { v < 1. }
    }

    pub fn gone(&self, now: Instant) -> bool {
        self.closing && self.value(now) <= 0.
    }

    pub fn close(&mut self, from: Option<Bounds<Pixels>>, now: Instant) {
        if self.closing {
            return;
        }
        self.at = self.value(now);
        self.since = now;
        self.closing = true;
        if from.is_some() {
            self.from = from;
        }
    }

    pub fn reopen(&mut self, now: Instant) {
        if !self.closing {
            return;
        }
        self.at = self.value(now);
        self.since = now;
        self.closing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    #[test]
    fn the_modal_card_keeps_inside_its_bounds_and_honours_the_requested_size() {
        assert_eq!(card_size((1200., 900.), None, None), (820., 792.));
        assert_eq!(card_size((300., 200.), None, None), (320., 240.));
        assert_eq!(card_size((800., 600.), None, None), (752., 528.));
        assert_eq!(card_size((1200., 900.), Some(500.), Some(400.)), (500., 400.));
        assert_eq!(card_size((1200., 900.), Some(100.), Some(2000.)), (320., 792.));
        assert_eq!(card_size((1200., 900.), Some(5000.), Some(10.)), (820., 240.));
    }

    #[test]
    fn mo1_the_modal_grows_from_the_anchor_and_shrinks_back_to_it() {
        let viewport = (1200., 900.);
        let end = end_rect(viewport, card_size(viewport, Some(500.), Some(400.)));
        let anchor = rect(100., 700., 20., 30.);
        let t0 = Instant::now();
        let mut m = Motion::open("t", Some(anchor), t0);
        let first = frame(m.from, end, m.t(t0));
        let (cx, cy) = centre(&first.card);
        assert!(f32::from(first.card.size.width) < 40.);
        assert!((cx - 110.).abs() < 30. && (cy - 715.).abs() < 30.);
        let open = t0 + OPEN;
        assert!(!m.moving(open));
        let settled = frame(m.from, end, m.t(open));
        assert!((centre(&settled.card).0 - 600.).abs() <= 2.);
        assert!((f32::from(settled.card.size.width) - 500.).abs() <= 2.);
        m.close(Some(anchor), open);
        let mid = open + Duration::from_millis(150);
        assert!(!m.gone(mid));
        let closing = frame(m.from, end, m.t(mid));
        assert!(f32::from(closing.card.size.width) < 500.);
        assert!(centre(&closing.card).1 > centre(&settled.card).1);
        assert!(m.gone(open + CLOSE));
    }

    #[test]
    fn without_an_anchor_the_modal_scales_from_ninety_seven_percent_in_the_middle() {
        let end = rect(100., 100., 400., 300.);
        let f = frame(None, end, 0.);
        assert!((f32::from(f.card.size.width) - 388.).abs() < 0.01);
        assert_eq!(centre(&f.card), centre(&end));
        assert_eq!((f.opacity, f.backdrop), (0., 0.));
        let done = frame(None, end, 1.);
        assert_eq!(done.card, end);
        assert_eq!((done.opacity, done.backdrop), (1., BACKDROP));
    }

    #[test]
    fn closing_halfway_through_opening_reverses_from_where_it_was() {
        let t0 = Instant::now();
        let mut m = Motion::open("t", None, t0);
        let half = t0 + OPEN / 2;
        let v = m.value(half);
        m.close(None, half);
        assert!((m.value(half) - v).abs() < 1e-6);
        assert!(m.gone(half + CLOSE.mul_f32(v)));
        m.reopen(half);
        assert!(!m.closing);
    }

    #[test]
    fn an_anchor_off_the_window_does_not_count() {
        assert!(on_screen(rect(10., 10., 5., 5.), (100., 100.)));
        assert!(!on_screen(rect(-20., 10., 5., 5.), (100., 100.)));
        assert!(!on_screen(rect(10., 10., 0., 5.), (100., 100.)));
    }
}
