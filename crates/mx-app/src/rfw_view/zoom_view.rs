use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, InteractiveElement, MouseButton, ParentElement, Pixels, Point,
    ScrollDelta, Styled, Task, WeakEntity, Window, canvas, px, relative,
};
use mx_rfw::Event;
use mx_rfw::widgets::{Local, decode};
use mx_rfw::widgets::zoom_view::{
    ANIMATION_MS, BoxFit, FittedBox, SETTLE_MS, Transform, ZoomView, command_transform, ease_out_cubic, fitted_scale,
    focus_in_placement, focus_transform, fitted_placement, move_extra, pan, should_report, wheel_factor, zoom_around, zoom_extra, zoom_target,
};

use super::ActionSink;
use super::layout::{Ctx, El, Fit, fire, owned, stretch_self};
use super::view::RfwView;
use super::widgets_core::optional;

// gpui reports a wheel notch as 3 lines; Flutter on Linux scrolls 53 px per notch.
const WHEEL_LINE_PX: f64 = 53.0 / 3.0;
const DEFAULT_TEXT: f32 = 13.0;

thread_local! {
    static SCALE: Cell<f32> = const { Cell::new(1.0) };
}

pub fn scale() -> f32 {
    SCALE.with(Cell::get)
}

pub fn scaled(v: f32) -> f32 {
    v * scale()
}

pub fn with_scale<T>(s: f32, f: impl FnOnce() -> T) -> T {
    let outer = SCALE.with(|c| c.replace(s));
    let out = f();
    SCALE.with(|c| c.set(outer));
    out
}

pub struct ZoomState {
    pub matrix: Transform,
    pub bounds: Option<Bounds<Pixels>>,
    rev: Option<i64>,
    anim: Option<(Instant, Transform, Transform)>,
    reported: f64,
    settle: Option<Task<()>>,
    grab: Option<Point<Pixels>>,
    moved: bool,
    /// Set when the direct child is a FittedBox: its slot, fit and alignment axes.
    content: Option<(FitSlot, BoxFit, (f64, f64))>,
}

impl Default for ZoomState {
    fn default() -> Self {
        Self { matrix: Transform::IDENTITY, bounds: None, rev: None, anim: None, reported: 1.0, settle: None, grab: None, moved: false, content: None }
    }
}

pub type ZoomSlot = Rc<RefCell<ZoomState>>;

impl ZoomState {
    fn size(&self) -> (f64, f64) {
        self.bounds.map_or((0.0, 0.0), |b| (f64::from(f32::from(b.size.width)), f64::from(f32::from(b.size.height))))
    }

    pub fn command(
        &mut self,
        command: Option<&str>,
        focus: Option<(f64, f64, f64, f64)>,
        rev: i64,
        max_scale: f64,
        now: Instant,
    ) {
        if self.rev.is_none() {
            self.rev = Some(rev);
            return;
        }
        if self.rev == Some(rev) {
            return;
        }
        self.rev = Some(rev);
        let current = self.current(now);
        if let (Some("focus"), Some(rect)) = (command, focus) {
            let rect = self.content_placement().map_or(rect, |placed| focus_in_placement(rect, placed));
            self.anim = Some((now, current, focus_transform(self.size(), rect, max_scale)));
        } else if let Some(to) = zoom_target(command, current.scale, max_scale) {
            self.anim = Some((now, current, command_transform(current, self.size(), to)));
        }
        if self.anim.is_some() {
            self.moved = false;
        }
    }

    fn content_placement(&self) -> Option<(f64, f64, f64, f64)> {
        let (slot, fit, align) = self.content.as_ref()?;
        let natural = slot.borrow().natural?;
        Some(fitted_placement(*fit, *align, (f64::from(natural.0), f64::from(natural.1)), self.size()))
    }

    pub fn current(&mut self, now: Instant) -> Transform {
        if let Some((start, from, to)) = self.anim {
            let t = now.duration_since(start).as_secs_f64() / Duration::from_millis(ANIMATION_MS).as_secs_f64();
            if t >= 1.0 {
                self.matrix = to;
                self.anim = None;
            } else {
                self.matrix = from.lerp(&to, ease_out_cubic(t));
            }
        }
        self.matrix
    }

    pub fn animating(&self) -> bool {
        self.anim.is_some()
    }

    fn interact(&mut self, m: Transform) {
        let changed = m != self.matrix;
        self.anim = None;
        self.matrix = m;
        if changed {
            self.moved = true;
        }
    }

    pub fn wheel(&mut self, at: Point<Pixels>, delta_y: f64, max_scale: f64) -> bool {
        let Some(b) = self.bounds else { return false };
        let focal = (f64::from(f32::from(at.x - b.origin.x)), f64::from(f32::from(at.y - b.origin.y)));
        let m = zoom_around(self.matrix, focal, wheel_factor(delta_y), max_scale, self.size());
        let changed = m != self.matrix;
        self.interact(m);
        changed
    }

    pub fn drag(&mut self, to: Point<Pixels>) -> bool {
        let Some(from) = self.grab.replace(to) else { return false };
        let (dx, dy) = (f64::from(f32::from(to.x - from.x)), f64::from(f32::from(to.y - from.y)));
        let m = pan(self.matrix, dx, dy, self.size());
        let changed = m != self.matrix;
        self.interact(m);
        changed
    }

    pub fn settled(&mut self) -> Option<f64> {
        let s = self.matrix.scale;
        if !should_report(s, self.reported) {
            return None;
        }
        self.reported = s;
        Some(s)
    }

    /// The transform to report through `onMove`, once a user pan or wheel has settled.
    pub fn settled_move(&mut self) -> Option<Transform> {
        if self.anim.is_some() || !std::mem::take(&mut self.moved) {
            return None;
        }
        Some(self.matrix)
    }
}

struct Reporter {
    slot: ZoomSlot,
    sink: ActionSink,
    events: Vec<Event>,
    move_events: Vec<Event>,
    view: WeakEntity<RfwView>,
}

impl Reporter {
    fn changed(&self, window: AnyWindowHandle, cx: &mut App) {
        let (slot, sink) = (self.slot.clone(), self.sink.clone());
        let (events, move_events) = (self.events.clone(), self.move_events.clone());
        let task = cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_millis(SETTLE_MS)).await;
            let (scale, moved) = {
                let mut st = slot.borrow_mut();
                (st.settled(), st.settled_move())
            };
            cx.update_window(window, |_, window, cx| {
                if let Some(scale) = scale.filter(|_| !events.is_empty()) {
                    fire(&sink, &events, &zoom_extra(scale), window, cx);
                }
                if let Some(m) = moved.filter(|_| !move_events.is_empty()) {
                    fire(&sink, &move_events, &move_extra(m), window, cx);
                }
            })
            .ok();
        });
        self.slot.borrow_mut().settle = Some(task);
        self.view.update(cx, |_, cx| cx.notify()).ok();
    }
}

// gpui drops window.refresh() calls made while the frame is being drawn.
fn refresh_after_draw(window: &Window, cx: &mut App) {
    let handle = window.window_handle();
    cx.defer(move |cx| {
        handle.update(cx, |_, window, _| window.refresh()).ok();
    });
}

fn line_delta(delta: ScrollDelta) -> (f64, bool) {
    match delta {
        ScrollDelta::Lines(p) => (-f64::from(p.y) * WHEEL_LINE_PX, true),
        ScrollDelta::Pixels(p) => (-f64::from(f32::from(p.y)), false),
    }
}

pub fn render(cx: &mut Ctx, z: &ZoomView, fit: Fit) -> El {
    let Some(states) = cx.widgets else { return stretch_self(optional(cx, z.child, fit).unwrap_or_else(|| cx.el())) };
    let slot = states.zooms.next();
    let now = Instant::now();
    let content = z.child.and_then(|w| match decode(w.as_node()?) {
        Local::FittedBox(f) => Some((states.fits.peek(), f.fit, super::style::align_axes(&f.alignment))),
        _ => None,
    });
    let m = {
        let mut st = slot.borrow_mut();
        st.content = content;
        st.command(z.command.as_deref(), z.focus, z.command_rev, z.max_scale, now);
        st.current(now)
    };
    let s = m.scale as f32;
    let inner = with_scale(s, || cx.within(true, |cx| optional(cx, z.child, Fit::BOTH)));
    let mut layer = cx.within(false, |cx| cx.el()).absolute().left(px(m.x as f32)).top(px(m.y as f32));
    layer = layer.w(relative(s)).h(relative(s)).text_size(px(DEFAULT_TEXT * s)).flex().flex_col();
    if let Some(i) = inner {
        layer = layer.child(super::layout::grow(i, 1.0));
    }
    let reporter = Rc::new(Reporter { slot: slot.clone(), sink: cx.sink.clone(), events: owned(&z.on_zoom), move_events: owned(&z.on_move), view: cx.view.clone() });
    let max = z.max_scale;
    let probe_slot = slot.clone();
    let probe_reporter = reporter.clone();
    let probe = canvas(
        move |bounds, window: &mut Window, cx: &mut App| {
            let mut st = probe_slot.borrow_mut();
            let resized = st.bounds.map(|b| b.size) != Some(bounds.size);
            st.bounds = Some(bounds);
            let animating = st.animating();
            drop(st);
            if animating {
                window.request_animation_frame();
                probe_reporter.changed(window.window_handle(), cx);
            } else if resized {
                refresh_after_draw(window, cx);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let mut el = stretch_self(cx.el().relative().overflow_hidden().size_full().child(layer).child(probe));
    if !cx.interactive {
        return el;
    }
    let (wheel_slot, wheel_rep) = (slot.clone(), reporter.clone());
    el = el.on_scroll_wheel(move |e, window, app| {
        let (dy, wheel) = line_delta(e.delta);
        let changed = if wheel || e.modifiers.control || e.modifiers.platform {
            wheel_slot.borrow_mut().wheel(e.position, dy, max)
        } else {
            let (dx, dy) = match e.delta {
                ScrollDelta::Pixels(p) => (p.x, p.y),
                ScrollDelta::Lines(_) => (px(0.), px(0.)),
            };
            let mut st = wheel_slot.borrow_mut();
            st.grab = Some(e.position);
            let moved = st.drag(e.position + gpui::point(dx, dy));
            st.grab = None;
            moved
        };
        if changed {
            app.stop_propagation();
            wheel_rep.changed(window.window_handle(), app);
        }
    });
    let down_slot = slot.clone();
    el = el.on_mouse_down(MouseButton::Left, move |e, _, _| {
        let mut st = down_slot.borrow_mut();
        st.anim = None;
        st.grab = Some(e.position);
    });
    let (move_slot, move_rep) = (slot.clone(), reporter);
    el = el.on_mouse_move(move |e, window, app| {
        if e.pressed_button != Some(MouseButton::Left) {
            return;
        }
        if move_slot.borrow_mut().drag(e.position) {
            move_rep.changed(window.window_handle(), app);
        }
    });
    let up_slot = slot;
    el.on_mouse_up(MouseButton::Left, move |_, _, _| up_slot.borrow_mut().grab = None)
}

#[derive(Default)]
pub struct FitState {
    outer: Option<(f32, f32)>,
    natural: Option<(f32, f32)>,
}

pub type FitSlot = Rc<RefCell<FitState>>;

pub fn fitted(cx: &mut Ctx, f: &FittedBox, fit: Fit) -> El {
    let Some(states) = cx.widgets else { return optional(cx, f.child, fit).unwrap_or_else(|| cx.el()) };
    let slot = states.fits.next();
    let (outer, natural) = {
        let st = slot.borrow();
        (st.outer, st.natural)
    };
    let s = match (outer, natural) {
        (Some(o), Some(n)) => {
            fitted_scale(f.fit, (f64::from(n.0), f64::from(n.1)), (f64::from(o.0), f64::from(o.1))) as f32
        }
        _ => scale(),
    };
    let inner = with_scale(s, || cx.within(false, |cx| optional(cx, f.child, Fit { w: false, h: false })));
    let (ax, ay) = super::style::align_axes(&f.alignment);
    let (left, top) = match (outer, natural) {
        (Some(o), Some(n)) => (
            (o.0 - n.0 * s) * (ax as f32 + 1.0) / 2.0,
            (o.1 - n.1 * s) * (ay as f32 + 1.0) / 2.0,
        ),
        _ => (0.0, 0.0),
    };
    let measured = natural.is_some();
    let used = s;
    let child_slot = slot.clone();
    let child_probe = canvas(
        move |bounds, window: &mut Window, cx: &mut App| {
            let n = (f32::from(bounds.size.width) / used, f32::from(bounds.size.height) / used);
            let mut st = child_slot.borrow_mut();
            let fresh = st.natural.is_none_or(|o| (o.0 - n.0).abs() > 0.5 || (o.1 - n.1).abs() > 0.5);
            if fresh && n.0 > 0.0 && n.1 > 0.0 {
                st.natural = Some(n);
                refresh_after_draw(window, cx);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let mut layer = cx.within(false, |cx| cx.el()).absolute().left(px(left)).top(px(top));
    layer = layer.text_size(px(DEFAULT_TEXT * s));
    if !measured {
        layer = layer.opacity(0.0);
    }
    layer = layer.children(inner).child(child_probe);
    let outer_probe = canvas(
        move |bounds, window: &mut Window, cx: &mut App| {
            let o = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let mut st = slot.borrow_mut();
            if st.outer != Some(o) {
                st.outer = Some(o);
                refresh_after_draw(window, cx);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    let mut el = cx.el().relative().size_full().child(layer).child(outer_probe);
    if f.clip {
        el = el.overflow_hidden();
    }
    stretch_self(el)
}

#[cfg(test)]
#[path = "zoom_view_tests.rs"]
mod tests;
