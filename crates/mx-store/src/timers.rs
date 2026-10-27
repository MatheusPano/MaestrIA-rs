use chrono::TimeDelta;
use mx_core::text::Time;

pub const SAVE_DEBOUNCE: TimeDelta = TimeDelta::milliseconds(400);
pub const SETTLE: TimeDelta = TimeDelta::seconds(3);
pub const BANNER_LIFE: TimeDelta = TimeDelta::seconds(6);
pub const TOAST_LIFE: TimeDelta = TimeDelta::seconds(8);
pub const REVEAL_LIFE: TimeDelta = TimeDelta::seconds(2);
pub const TICK_FAST: TimeDelta = TimeDelta::seconds(1);
pub const TICK_GIT: TimeDelta = TimeDelta::seconds(10);
pub const TICK_AGES: TimeDelta = TimeDelta::seconds(20);
pub const AGENTS_EAGER: TimeDelta = TimeDelta::seconds(2);
pub const AGENTS_IDLE: TimeDelta = TimeDelta::seconds(30);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimerKey {
    Save,
    Banner,
    Reveal,
    Toast(u64),
    Settle(String),
    TickFast,
    TickGit,
    TickAges,
    Agents,
}

#[derive(Debug, Default)]
pub(crate) struct Timers {
    due: Vec<(TimerKey, Time)>,
}

impl Timers {
    pub fn set(&mut self, key: TimerKey, at: Time) {
        self.cancel(&key);
        self.due.push((key, at));
    }

    pub fn cancel(&mut self, key: &TimerKey) -> bool {
        let before = self.due.len();
        self.due.retain(|(k, _)| k != key);
        before != self.due.len()
    }

    pub fn is_set(&self, key: &TimerKey) -> bool {
        self.due.iter().any(|(k, _)| k == key)
    }

    pub fn next(&self) -> Option<Time> {
        self.due.iter().map(|(_, t)| *t).min()
    }

    pub fn take_due(&mut self, now: Time) -> Vec<TimerKey> {
        let mut fired: Vec<(TimerKey, Time)> = Vec::new();
        self.due.retain(|(k, t)| {
            if *t <= now {
                fired.push((k.clone(), *t));
                false
            } else {
                true
            }
        });
        fired.sort_by_key(|(_, t)| *t);
        fired.into_iter().map(|(k, _)| k).collect()
    }

    pub fn clear(&mut self) {
        self.due.clear();
    }
}
