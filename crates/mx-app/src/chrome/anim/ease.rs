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
