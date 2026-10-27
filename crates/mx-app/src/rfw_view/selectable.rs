use std::rc::Rc;

use gpui::{App, Hsla, IntoElement, SharedString};
use mx_rfw::widgets::maestria::SelectionArea;
use mx_theme::Palette;

use super::layout::{Ctx, El, Fit, pass};
use super::style::alpha;
use super::widgets_core::child;
use crate::reader::select::{self, Notify, area};

pub use crate::reader::select::{SelectionSlot, SelectionState};

pub fn selection_color(p: &Palette) -> Hsla {
    alpha(p.accent, 0.35)
}

pub fn notifier(cx: &Ctx) -> Notify {
    let view = cx.view.clone();
    Rc::new(move |app: &mut App| {
        view.update(app, |_, cx| cx.notify()).ok();
    })
}

pub fn render(cx: &mut Ctx, s: &SelectionArea, fit: Fit) -> El {
    let Some(states) = cx.widgets else {
        let inner = child(cx, s.child.clone(), fit);
        return pass(cx.el(), Some(inner));
    };
    if s.lines_only {
        let inner = child(cx, s.child.clone(), fit);
        return pass(cx.el(), Some(inner));
    }
    let slot = states.selections.next();
    {
        let mut st = slot.borrow_mut();
        st.begin_frame();
        st.color = Some(selection_color(cx.palette));
    }
    let outer = states.current_selection.replace(Some(slot.clone()));
    let inner = child(cx, s.child.clone(), fit);
    states.current_selection.replace(outer);
    slot.borrow_mut().end_frame();
    let el = pass(cx.el(), Some(inner));
    if cx.interactive { area(el, &slot, notifier(cx)) } else { el }
}

pub fn text(cx: &Ctx, text: SharedString) -> Option<gpui::AnyElement> {
    let slot = cx.widgets?.selection()?;
    Some(select::piece(&slot, text, None).into_any_element())
}
