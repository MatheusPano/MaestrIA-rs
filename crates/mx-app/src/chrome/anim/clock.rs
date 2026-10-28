use std::sync::LazyLock;
use std::time::Instant;

static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

pub fn ms_at(at: Instant) -> u64 {
    at.saturating_duration_since(*EPOCH).as_millis() as u64
}

pub fn now_ms() -> u64 {
    ms_at(Instant::now())
}

pub fn until_next(step_ms: u64, now_ms: u64) -> u64 {
    step_ms - now_ms % step_ms
}

#[cfg(test)]
mod tests {
    use super::until_next;

    #[test]
    fn waits_land_on_the_following_step_boundary() {
        assert_eq!(until_next(150, 0), 150);
        assert_eq!(until_next(150, 470), 130);
        assert_eq!(until_next(50, 449), 1);
    }
}
