use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use gpui::{
    AlignItems, Div, ElementId, Hsla, InteractiveElement, ParentElement, Stateful, Styled, WeakEntity, div, px,
};
use mx_rfw::{Event, Map};
use std::sync::Arc;

use mx_theme::Theme;

use super::view::RfwView;
use super::{ActionSink, ViewAction};

pub type El = Stateful<Div>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    pub w: bool,
    pub h: bool,
}

impl Fit {
    pub const BOTH: Fit = Fit { w: true, h: true };

    pub fn any(self) -> bool {
        self.w || self.h
    }
}

#[derive(Debug, Clone)]
pub struct DropInfo {
    pub id: String,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DragNow {
    pub payload: String,
    pub targets: Vec<String>,
}

impl DragNow {
    pub fn fits(&self, id: &str) -> bool {
        self.targets.is_empty() || self.targets.iter().any(|t| t == id)
    }
}

pub struct Ctx<'a> {
    pub palette: &'a Arc<Theme>,
    pub dir: Option<&'a Path>,
    pub sink: ActionSink,
    pub view: WeakEntity<RfwView>,
    pub ids: u64,
    pub drops: Vec<Rc<DropInfo>>,
    pub dragging: Option<DragNow>,
    pub turns: f64,
    pub measured: &'a HashMap<u64, f32>,
    pub interactive: bool,
}

impl Ctx<'_> {
    pub fn next_id(&mut self) -> u64 {
        self.ids += 1;
        self.ids
    }

    pub fn el(&mut self) -> El {
        let id = self.next_id();
        div().id(ElementId::NamedInteger("rfw".into(), id))
    }

    pub fn measured(&self, id: u64) -> Option<f32> {
        self.measured.get(&id).copied()
    }
}

pub fn fire(sink: &ActionSink, events: &[Event], extra: &Map, window: &mut gpui::Window, cx: &mut gpui::App) {
    for e in events {
        sink(ViewAction { action: e.name.clone(), values: e.values(extra) }, window, cx);
    }
}

pub fn owned(events: &[&Event]) -> Vec<Event> {
    events.iter().map(|e| (*e).clone()).collect()
}

pub fn grow(mut el: El, flex: f32) -> El {
    let s = el.style();
    s.flex_grow = Some(flex);
    s.flex_shrink = Some(1.0);
    s.flex_basis = Some(px(0.0).into());
    s.min_size.width = Some(px(0.0).into());
    s.min_size.height = Some(px(0.0).into());
    el
}

pub fn rigid(mut el: El) -> El {
    el.style().flex_shrink = Some(0.0);
    el
}

pub fn stretch_self(mut el: El) -> El {
    el.style().align_self = Some(AlignItems::Stretch);
    el
}

pub fn pass(mut el: El, child: Option<El>) -> El {
    let s = el.style();
    s.display = Some(gpui::Display::Flex);
    s.flex_direction = Some(gpui::FlexDirection::Column);
    s.align_items = Some(AlignItems::Stretch);
    match child {
        Some(mut c) => {
            c.style().flex_grow = Some(1.0);
            el.child(c)
        }
        None => el,
    }
}

pub fn error_text(cx: &mut Ctx, message: &str, color: Hsla) -> El {
    cx.el()
        .p(px(4.0))
        .text_size(px(10.5))
        .text_color(color)
        .font_family(super::view::mono_family())
        .child(message.to_owned())
}
