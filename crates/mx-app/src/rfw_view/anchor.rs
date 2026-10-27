use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{Bounds, Pixels, ParentElement, Styled, canvas};
use mx_rfw::widgets::maestria::Anchor;

use super::layout::{Ctx, El, Fit, pass};
use super::widgets_core::child;

thread_local! {
    static ANCHORS: RefCell<HashMap<String, (Bounds<Pixels>, u64)>> = RefCell::new(HashMap::new());
}

static OWNERS: AtomicU64 = AtomicU64::new(1);

pub fn new_owner() -> u64 {
    OWNERS.fetch_add(1, Ordering::Relaxed)
}

pub fn anchor_rect(key: &str) -> Option<Bounds<Pixels>> {
    ANCHORS.with(|a| a.borrow().get(key).map(|(b, _)| *b))
}

pub fn register(key: &str, bounds: Bounds<Pixels>, owner: u64) {
    ANCHORS.with(|a| a.borrow_mut().insert(key.to_owned(), (bounds, owner)));
}

pub fn forget_unseen(owner: u64, seen: &HashSet<String>) {
    ANCHORS.with(|a| a.borrow_mut().retain(|k, (_, o)| *o != owner || seen.contains(k)));
}

pub fn forget_owner(owner: u64) {
    forget_unseen(owner, &HashSet::new());
}

pub fn render(cx: &mut Ctx, a: &Anchor, fit: Fit) -> El {
    let inner = child(cx, a.child.clone(), fit);
    let el = pass(cx.el().relative(), Some(inner));
    let Some(states) = cx.widgets else { return el };
    let key = a.key(&states.plugin_id);
    states.anchors.borrow_mut().insert(key.clone());
    let owner = states.owner;
    el.child(canvas(move |bounds, _, _| register(&key, bounds, owner), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
}

#[cfg(test)]
mod tests {
    use gpui::{point, px, size};

    use super::*;

    #[test]
    fn last_registered_wins_and_only_the_owner_forgets_its_anchors() {
        let (a, b) = (new_owner(), new_owner());
        let r1 = Bounds::new(point(px(1.), px(2.)), size(px(3.), px(4.)));
        let r2 = Bounds::new(point(px(5.), px(6.)), size(px(7.), px(8.)));
        register("t/x", r1, a);
        register("t/x", r2, b);
        assert_eq!(anchor_rect("t/x"), Some(r2));
        forget_owner(a);
        assert_eq!(anchor_rect("t/x"), Some(r2));
        let mut seen = HashSet::new();
        seen.insert("t/y".to_owned());
        forget_unseen(b, &seen);
        assert_eq!(anchor_rect("t/x"), None);
    }
}
