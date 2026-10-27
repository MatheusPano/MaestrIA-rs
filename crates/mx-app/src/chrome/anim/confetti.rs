use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Fire,
    Puff,
}

impl Variant {
    pub fn life_ms(self) -> u64 {
        match self {
            Variant::Fire => 2600,
            Variant::Puff => 1400,
        }
    }

    pub fn fade_from(self) -> f32 {
        match self {
            Variant::Fire => 0.72,
            Variant::Puff => 0.62,
        }
    }
}

pub const GRAVITY: f32 = 1.5;
pub const DRAG: f32 = 1.15;
pub const COLORS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub ox: f32,
    pub oy: f32,
    pub angle: f32,
    pub speed: f32,
    pub spin: f32,
    pub flutter: f32,
    pub phase: f32,
    pub w: f32,
    pub h: f32,
    pub delay: f32,
    pub color: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub alpha: f32,
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    pub fn below(&mut self, n: usize) -> usize {
        ((self.next_f32() * n as f32) as usize).min(n - 1)
    }
}

pub fn spawn(variant: Variant, origin: (f32, f32), rng: &mut Rng) -> Vec<Particle> {
    match variant {
        Variant::Fire => {
            let mut out = Vec::with_capacity(110);
            for left in [true, false] {
                for _ in 0..55 {
                    let up = rng.range(0.19, 0.44) * PI;
                    out.push(Particle {
                        ox: if left { 0. } else { 1. },
                        oy: 1.,
                        angle: if left { up } else { PI - up },
                        speed: rng.range(2.1, 3.1),
                        spin: rng.range(-9., 9.),
                        flutter: rng.range(5., 11.),
                        phase: rng.range(0., 2. * PI),
                        w: rng.range(5., 9.),
                        h: rng.range(3., 12.),
                        delay: rng.range(0., 0.16),
                        color: rng.below(COLORS),
                    });
                }
            }
            out
        }
        Variant::Puff => (0..40)
            .map(|_| Particle {
                ox: origin.0,
                oy: origin.1,
                angle: rng.range(0., 2. * PI),
                speed: rng.range(0.6, 1.5),
                spin: rng.range(-11., 11.),
                flutter: rng.range(6., 13.),
                phase: rng.range(0., 2. * PI),
                w: rng.range(4., 7.),
                h: rng.range(3., 8.),
                delay: 0.,
                color: rng.below(COLORS),
            })
            .collect(),
    }
}

pub fn pose(p: &Particle, variant: Variant, elapsed: f32, width: f32, height: f32) -> Option<Pose> {
    let t = elapsed - p.delay;
    if t <= 0. {
        return None;
    }
    let decay = (1. - (-DRAG * t).exp()) / DRAG;
    let vt = GRAVITY * height / DRAG;
    let vx = p.angle.cos() * p.speed * height;
    let vy = -p.angle.sin() * p.speed * height;
    let x = p.ox * width + vx * decay;
    let y = p.oy * height + (vy - vt) * decay + vt * t;
    if y > height + 40. {
        return None;
    }
    let progress = elapsed * 1000. / variant.life_ms() as f32;
    let fade = variant.fade_from();
    let alpha = if progress < fade { 1. } else { (1. - (progress - fade) / (1. - fade)).clamp(0., 1.) };
    Some(Pose {
        x,
        y,
        rotation: p.spin * t,
        scale_x: (p.phase + p.flutter * t).cos(),
        alpha,
    })
}
