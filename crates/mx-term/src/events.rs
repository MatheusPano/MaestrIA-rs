use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_channel::{Receiver, Sender};

use crate::session::TermEvent;

const CAPACITY: usize = 256;

#[derive(Debug)]
struct Flags {
    dirty: AtomicBool,
    queued: AtomicBool,
}

#[derive(Clone, Debug)]
pub struct DirtyFlag(Arc<Flags>);

impl DirtyFlag {
    pub fn take(&self) -> bool {
        self.0.queued.store(false, Ordering::Release);
        self.0.dirty.swap(false, Ordering::AcqRel)
    }
}

pub struct Subscriber {
    pub events: Receiver<TermEvent>,
    pub dirty: DirtyFlag,
}

struct Sub {
    tx: Sender<TermEvent>,
    flags: Arc<Flags>,
}

pub(crate) struct Fanout {
    primary: Sender<TermEvent>,
    primary_dirty: AtomicBool,
    subs: Mutex<Vec<Sub>>,
}

impl Fanout {
    pub fn new() -> (Self, Receiver<TermEvent>) {
        let (tx, rx) = async_channel::bounded(CAPACITY);
        let fanout = Self { primary: tx, primary_dirty: AtomicBool::new(false), subs: Mutex::new(Vec::new()) };
        (fanout, rx)
    }

    pub fn subscribe(&self) -> Subscriber {
        let (tx, rx) = async_channel::bounded(CAPACITY);
        let flags = Arc::new(Flags { dirty: AtomicBool::new(true), queued: AtomicBool::new(false) });
        if let Ok(mut subs) = self.subs.lock() {
            subs.retain(|s| !s.tx.is_closed());
            subs.push(Sub { tx, flags: flags.clone() });
        }
        Subscriber { events: rx, dirty: DirtyFlag(flags) }
    }

    pub fn wake(&self) {
        if !self.primary_dirty.swap(true, Ordering::AcqRel) {
            let _ = self.primary.try_send(TermEvent::Wakeup);
        }
        let Ok(mut subs) = self.subs.lock() else { return };
        subs.retain(|s| !s.tx.is_closed());
        for s in subs.iter() {
            s.flags.dirty.store(true, Ordering::Release);
            if !s.flags.queued.swap(true, Ordering::AcqRel) && s.tx.try_send(TermEvent::Wakeup).is_err() {
                s.flags.queued.store(false, Ordering::Release);
            }
        }
    }

    pub fn emit(&self, event: TermEvent) {
        if let Ok(mut subs) = self.subs.lock() {
            subs.retain(|s| !s.tx.is_closed());
            for s in subs.iter() {
                let _ = s.tx.try_send(event.clone());
            }
        }
        let _ = self.primary.try_send(event);
    }

    pub fn take_dirty(&self) -> bool {
        self.primary_dirty.swap(false, Ordering::AcqRel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(rx: &Receiver<TermEvent>) -> Vec<TermEvent> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[test]
    fn every_subscriber_gets_its_own_wakeup() {
        let (f, _primary) = Fanout::new();
        let a = f.subscribe();
        let b = f.subscribe();
        f.wake();
        assert_eq!(drain(&a.events), [TermEvent::Wakeup]);
        assert_eq!(drain(&b.events), [TermEvent::Wakeup]);
    }

    #[test]
    fn a_dropped_subscriber_does_not_swallow_the_next_wakeup() {
        let (f, _primary) = Fanout::new();
        let old = f.subscribe();
        f.wake();
        drop(old);
        let new = f.subscribe();
        f.wake();
        assert_eq!(drain(&new.events), [TermEvent::Wakeup]);
    }

    #[test]
    fn wakeups_coalesce_until_the_frame_takes_the_flag() {
        let (f, _primary) = Fanout::new();
        let s = f.subscribe();
        f.wake();
        f.wake();
        assert_eq!(drain(&s.events), [TermEvent::Wakeup]);
        f.wake();
        assert!(drain(&s.events).is_empty());
        assert!(s.dirty.take());
        assert!(!s.dirty.take());
        f.wake();
        assert_eq!(drain(&s.events), [TermEvent::Wakeup]);
    }

    #[test]
    fn a_full_channel_does_not_wedge_later_wakeups() {
        let (f, _primary) = Fanout::new();
        let s = f.subscribe();
        for _ in 0..CAPACITY {
            f.emit(TermEvent::Bell);
        }
        f.wake();
        drain(&s.events);
        f.wake();
        assert_eq!(drain(&s.events), [TermEvent::Wakeup]);
    }

    #[test]
    fn a_new_subscriber_starts_dirty() {
        let (f, _primary) = Fanout::new();
        assert!(f.subscribe().dirty.take());
    }

    #[test]
    fn events_reach_every_subscriber_and_the_primary_channel() {
        let (f, primary) = Fanout::new();
        let a = f.subscribe();
        f.emit(TermEvent::Title("x".into()));
        assert_eq!(drain(&a.events), [TermEvent::Title("x".into())]);
        assert_eq!(drain(&primary), [TermEvent::Title("x".into())]);
    }
}
