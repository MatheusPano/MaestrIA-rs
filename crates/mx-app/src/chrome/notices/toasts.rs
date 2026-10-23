use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{Context, InteractiveElement, IntoElement, ParentElement, Render, Styled, Window, div, px};

use super::anim_off;
use super::row::{NoticeView, RowHandlers, notice_row};
use crate::chrome::anim::ease::{ease_in_cubic, ease_out, ease_out_cubic, interval};
use crate::chrome::style::{alpha, hsla, shadow, theme};

pub const TOAST_LIFE: Duration = Duration::from_secs(8);
pub const TOAST_ENTER: Duration = Duration::from_millis(280);
pub const TOAST_WIDTH: f32 = 380.;

struct Entry {
    view: NoticeView,
    born: Instant,
    leaving: Option<Instant>,
}

pub struct ToastStack {
    entries: Vec<Entry>,
    handlers: Rc<RowHandlers>,
    from_top: bool,
}

pub fn enter_frame(p: f32) -> (f32, f32, f32) {
    let size = ease_out(interval(p, 0., 0.45));
    let slide = ease_out_cubic(interval(p, 0.25, 1.));
    (size, (1. - slide) * 1.15 * TOAST_WIDTH, slide)
}

pub fn exit_frame(p: f32) -> (f32, f32, f32) {
    let q = 1. - p;
    let size = ease_in_cubic(interval(q, 0., 0.45));
    let slide = ease_in_cubic(interval(q, 0.25, 1.));
    (size, (1. - slide) * 1.15 * TOAST_WIDTH, slide)
}

impl ToastStack {
    pub fn new(handlers: RowHandlers) -> Self {
        Self { entries: Vec::new(), handlers: Rc::new(handlers), from_top: false }
    }

    pub fn set_on_top(&mut self, top: bool, cx: &mut Context<Self>) {
        self.from_top = top;
        cx.notify();
    }

    pub fn ids(&self) -> Vec<u64> {
        self.entries.iter().map(|e| e.view.id).collect()
    }

    pub fn set(&mut self, views: Vec<NoticeView>, cx: &mut Context<Self>) {
        let now = Instant::now();
        for e in &mut self.entries {
            if e.leaving.is_none() && !views.iter().any(|v| v.id == e.view.id) {
                e.leaving = Some(now);
                let id = e.view.id;
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(TOAST_ENTER).await;
                    this.update(cx, |this, cx| {
                        this.entries.retain(|e| e.view.id != id || e.leaving.is_none());
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
        }
        for v in views {
            match self.entries.iter_mut().find(|e| e.view.id == v.id) {
                Some(e) => e.view = v,
                None => self.entries.push(Entry { view: v, born: now, leaving: None }),
            }
        }
        cx.notify();
    }
}

impl Render for ToastStack {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let now = Instant::now();
        let total = TOAST_ENTER.as_secs_f32();
        let mut animating = false;
        let mut cards = Vec::new();
        for e in &self.entries {
            let (size, dx, fade) = if anim_off() {
                if e.leaving.is_some() { (0., 0., 0.) } else { (1., 0., 1.) }
            } else {
                match e.leaving {
                    Some(at) => exit_frame((now.duration_since(at).as_secs_f32() / total).min(1.)),
                    None => enter_frame((now.duration_since(e.born).as_secs_f32() / total).min(1.)),
                }
            };
            animating |= size < 1. || fade < 1. || e.leaving.is_some();
            let id = e.view.id;
            let card = div()
                .id(("toast", id as usize))
                .debug_selector(move || format!("toast:{id}"))
                .relative()
                .left(px(dx))
                .opacity(fade)
                .w(px(TOAST_WIDTH))
                .bg(hsla(t.bg_sidebar))
                .rounded(px(8.))
                .border_1()
                .border_color(alpha(t.accent, 0.7))
                .shadow(shadow(&t, 18., 6.))
                .overflow_hidden()
                .child(notice_row(&e.view, true, true, &self.handlers, &t, cx));
            let gap = if self.from_top { div().pt(px(8. * size)) } else { div().pb(px(8. * size)) };
            cards.push(gap.max_h(px(160. * size)).overflow_hidden().child(card));
        }
        if !self.from_top {
            cards.reverse();
        }
        if animating {
            window.request_animation_frame();
        }
        div().flex().flex_col().items_end().children(cards)
    }
}
