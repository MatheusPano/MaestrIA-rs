use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, InteractiveElement, IntoElement, ParentElement, Pixels,
    Point, Render, ScrollHandle, StatefulInteractiveElement, Styled, Window, anchored, canvas,
    deferred, div, point, px,
};
use mx_theme::Theme;

use super::geometry::{self, Rect};
use super::model::{MenuEntry, MenuItem, content_height};
use super::row::{caption, divider, item_row};
use crate::chrome::style::{Callback, Handler, hsla, shadow, theme};

type Frame = Rc<Cell<Option<Bounds<Pixels>>>>;

pub struct ContextMenu<T> {
    entries: Vec<MenuEntry<T>>,
    anchor: Point<Pixels>,
    min_width: f32,
    max_width: f32,
    on_pick: Handler<T>,
    on_dismiss: Callback,
    open_sub: Option<usize>,
    frame: Frame,
    sub_frame: Frame,
    scroll: ScrollHandle,
    sub_scroll: ScrollHandle,
}

impl<T: Clone + 'static> ContextMenu<T> {
    pub fn new(
        entries: Vec<MenuEntry<T>>,
        anchor: Point<Pixels>,
        on_pick: impl Fn(T, &mut Window, &mut App) + 'static,
        on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            entries,
            anchor,
            min_width: 216.,
            max_width: 340.,
            on_pick: Rc::new(on_pick),
            on_dismiss: Rc::new(on_dismiss),
            open_sub: None,
            frame: Rc::default(),
            sub_frame: Rc::default(),
            scroll: ScrollHandle::new(),
            sub_scroll: ScrollHandle::new(),
        }
    }

    pub fn set_entries(&mut self, entries: Vec<MenuEntry<T>>, cx: &mut Context<Self>) {
        self.entries = entries;
        cx.notify();
    }

    pub fn preopen(&mut self, index: Option<usize>) {
        self.open_sub = index;
    }

    #[cfg(test)]
    pub fn open_submenu(&self) -> Option<usize> {
        self.open_sub
    }

    fn pick(&mut self, item: &MenuItem<T>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(value) = item.value.clone() else {
            return;
        };
        (self.on_pick)(value, window, cx);
        if !item.keep_open {
            (self.on_dismiss)(window, cx);
        }
    }

    fn entry(
        &self,
        index: usize,
        entry: &MenuEntry<T>,
        sub: bool,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = if sub { 1000 + index } else { index };
        match entry {
            MenuEntry::Divider => divider(t),
            MenuEntry::Caption(text) => caption(text, t),
            MenuEntry::Item(item) => {
                let picked = item.clone();
                let mut row = item_row(id, item, t, false, false);
                if !sub {
                    row = row.on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if *hovered && this.open_sub.is_some() {
                            this.open_sub = None;
                            cx.notify();
                        }
                    }));
                }
                if !item.disabled {
                    row = row.on_click(cx.listener(move |this, _, window, cx| {
                        this.pick(&picked, window, cx)
                    }));
                }
                row.into_any_element()
            }
            MenuEntry::Submenu(item, _) => {
                let open = self.open_sub == Some(index);
                let mut row = item_row(id, item, t, true, open);
                if !item.disabled {
                    row = row
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if *hovered && this.open_sub != Some(index) {
                                this.open_sub = Some(index);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_sub = Some(index);
                            cx.notify();
                        }));
                }
                row.into_any_element()
            }
        }
    }

    fn panel(&self, t: &Theme, max_h: f32, scroll: &ScrollHandle, frame: Frame) -> gpui::Div {
        div()
            .relative()
            .bg(hsla(t.bg_active))
            .rounded(px(10.))
            .border_1()
            .border_color(hsla(t.border))
            .shadow(shadow(t, 24., 8.))
            .max_h(px(max_h))
            .child(
                canvas(move |b, _, _| frame.set(Some(b)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(div().id("menu-scroll-anchor").track_scroll(scroll))
    }

    fn submenu(&self, t: &Theme, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let index = self.open_sub?;
        let MenuEntry::Submenu(_, children) = self.entries.get(index)? else {
            return None;
        };
        let parent = self.frame.get()?;
        let viewport = window.viewport_size();
        let scroll_y = f32::from(self.scroll.offset().y);
        let row_top = f32::from(parent.origin.y)
            + scroll_y
            + geometry::row_offset(self.entries.iter().map(MenuEntry::height), index);
        let frame = geometry::submenu_frame(
            to_rect(parent),
            row_top,
            content_height(children),
            f32::from(viewport.width),
            f32::from(viewport.height),
        );
        let rows: Vec<AnyElement> = children
            .iter()
            .enumerate()
            .map(|(i, e)| self.entry(i, e, true, t, cx))
            .collect();
        let panel = self
            .panel(t, frame.h, &self.sub_scroll, self.sub_frame.clone())
            .debug_selector(|| "submenu".into())
            .w(px(geometry::SUBMENU_WIDTH))
            .h(px(frame.h))
            .child(
                div()
                    .id("submenu-scroll")
                    .size_full()
                    .py(px(geometry::PAD))
                    .overflow_y_scroll()
                    .track_scroll(&self.sub_scroll)
                    .children(rows),
            );
        Some(
            deferred(
                anchored()
                    .position(point(px(frame.x), px(frame.y)))
                    .child(panel),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

fn to_rect(b: Bounds<Pixels>) -> Rect {
    Rect::new(
        f32::from(b.origin.x),
        f32::from(b.origin.y),
        f32::from(b.size.width),
        f32::from(b.size.height),
    )
}

impl<T: Clone + 'static> Render for ContextMenu<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let max_h = geometry::menu_max_height(f32::from(window.viewport_size().height));
        let rows: Vec<AnyElement> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| self.entry(i, e, false, &t, cx))
            .collect();
        let sub_frame = self.sub_frame.clone();
        let has_sub = self.open_sub.is_some();
        let panel = self
            .panel(&t, max_h, &self.scroll, self.frame.clone())
            .debug_selector(|| "menu".into())
            .min_w(px(self.min_width))
            .max_w(px(self.max_width))
            .on_mouse_down_out(cx.listener(move |this, ev: &gpui::MouseDownEvent, window, cx| {
                let inside_sub = has_sub
                    && sub_frame.get().is_some_and(|b| b.contains(&ev.position));
                if !inside_sub {
                    (this.on_dismiss)(window, cx);
                }
            }))
            .child(
                div()
                    .id("menu-scroll")
                    .max_h(px(max_h))
                    .py(px(geometry::PAD))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(rows),
            );
        let sub = self.submenu(&t, window, cx);
        div()
            .child(
                deferred(
                    anchored()
                        .position(self.anchor)
                        .snap_to_window_with_margin(px(geometry::MARGIN))
                        .child(panel),
                )
                .with_priority(1),
            )
            .children(sub)
    }
}
