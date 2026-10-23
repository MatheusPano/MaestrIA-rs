pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(2)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

pub fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 { 2. * t * t } else { 1. - (-2. * t + 2.).powi(2) / 2. }
}

pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4. * t * t * t } else { 1. - (-2. * t + 2.).powi(3) / 2. }
}

pub fn interval(t: f32, begin: f32, end: f32) -> f32 {
    ((t - begin) / (end - begin)).clamp(0., 1.)
}

pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub start: std::time::Instant,
    pub millis: f32,
}

impl Tween {
    pub fn new(from: f32, to: f32, millis: f32) -> Self {
        Self { from, to, start: std::time::Instant::now(), millis }
    }

    pub fn at(&self, now: std::time::Instant) -> (f32, bool) {
        let t = (now.duration_since(self.start).as_secs_f32() * 1000. / self.millis).clamp(0., 1.);
        (self.from + (self.to - self.from) * ease_out(t), t >= 1.)
    }
}
