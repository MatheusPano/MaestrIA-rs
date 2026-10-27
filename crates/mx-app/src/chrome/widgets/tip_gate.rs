use std::time::{Duration, Instant};

use gpui::{
    AnyView, App, AppContext, Context, DispatchPhase, Global, IntoElement, MouseDownEvent, Render, Styled,
    Window, canvas,
};

const SHOW_DELAY: Duration = Duration::from_millis(500);
const SLACK: Duration = Duration::from_millis(250);

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Overlays {
    pub generation: u64,
    pub count: usize,
    pub menu_on_top: bool,
}

#[derive(Default)]
struct TipGate {
    last_down: Option<Instant>,
    overlays: Overlays,
}

impl Global for TipGate {}

#[derive(Clone, Copy, Debug)]
struct Stamp {
    born: Instant,
    overlays: Overlays,
    muted: bool,
}

fn stamp(gate: &TipGate, now: Instant) -> Stamp {
    let clicked_while_waiting = gate.last_down.is_some_and(|t| now.saturating_duration_since(t) < SHOW_DELAY + SLACK);
    Stamp { born: now, overlays: gate.overlays, muted: clicked_while_waiting || gate.overlays.menu_on_top }
}

fn hidden(gate: &TipGate, s: &Stamp) -> bool {
    s.muted || gate.overlays != s.overlays || gate.last_down.is_some_and(|t| t >= s.born)
}

pub fn set_overlays(overlays: Overlays, cx: &mut App) {
    cx.default_global::<TipGate>().overlays = overlays;
}

pub fn watch_clicks() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |_, _, window, _| {
            window.on_mouse_event(|_: &MouseDownEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture {
                    cx.default_global::<TipGate>().last_down = Some(Instant::now());
                }
            })
        },
    )
    .absolute()
    .size_0()
}

struct Gated {
    inner: AnyView,
    stamp: Stamp,
}

impl Render for Gated {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let gone = cx.try_global::<TipGate>().is_some_and(|g| hidden(g, &self.stamp));
        if gone { gpui::Empty.into_any_element() } else { self.inner.clone().into_any_element() }
    }
}

pub fn gated(
    build: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |window, cx| {
        let inner = build(window, cx);
        let stamp = stamp(cx.default_global::<TipGate>(), Instant::now());
        cx.new(|_| Gated { inner, stamp }).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(down_ago_ms: Option<u64>, now: Instant) -> TipGate {
        TipGate { last_down: down_ago_ms.map(|ms| now - Duration::from_millis(ms)), overlays: Overlays::default() }
    }

    #[test]
    fn a_tooltip_that_was_waiting_to_show_when_its_trigger_was_clicked_never_appears() {
        let now = Instant::now();
        assert!(hidden(&gate(Some(400), now), &stamp(&gate(Some(400), now), now)));
    }

    #[test]
    fn a_tooltip_after_a_quiet_hover_shows() {
        let now = Instant::now();
        let g = gate(Some(5_000), now);
        assert!(!hidden(&g, &stamp(&g, now)));
        assert!(!hidden(&gate(None, now), &stamp(&gate(None, now), now)));
    }

    #[test]
    fn clicking_hides_a_visible_tooltip() {
        let now = Instant::now();
        let s = stamp(&gate(None, now), now);
        let later = TipGate { last_down: Some(now + Duration::from_millis(10)), overlays: Overlays::default() };
        assert!(hidden(&later, &s));
    }

    #[test]
    fn opening_an_overlay_hides_a_visible_tooltip() {
        let now = Instant::now();
        let s = stamp(&gate(None, now), now);
        let mut g = gate(None, now);
        g.overlays = Overlays { generation: 1, count: 1, menu_on_top: false };
        assert!(hidden(&g, &s));
    }

    #[test]
    fn no_tooltip_shows_while_a_menu_is_open() {
        let now = Instant::now();
        let mut g = gate(None, now);
        g.overlays = Overlays { generation: 3, count: 1, menu_on_top: true };
        assert!(hidden(&g, &stamp(&g, now)));
    }

    #[test]
    fn a_tooltip_inside_a_dialog_shows() {
        let now = Instant::now();
        let mut g = gate(None, now);
        g.overlays = Overlays { generation: 3, count: 1, menu_on_top: false };
        assert!(!hidden(&g, &stamp(&g, now)));
    }
}
