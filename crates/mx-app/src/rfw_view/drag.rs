use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AppContext, BoxShadow, Context, CursorStyle, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, div, point, prelude::*, px,
};
use mx_rfw::Widget;
use mx_rfw::widgets::maestria::{Draggable, DropTarget};
use mx_theme::Theme;

use super::layout::{Ctx, DragNow, DropInfo, El, Fit, owned, pass};
use super::style::{alpha, hsla, on_accent, set_radius};
use super::view::{RfwView, text_root};
use super::widgets_core::{optional, render};

pub struct Preview {
    widget: Option<Widget>,
    theme: Arc<Theme>,
    dir: Option<PathBuf>,
    width: f32,
    view: WeakEntity<RfwView>,
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let measured = HashMap::new();
        let mut cx = Ctx {
            palette: &self.theme,
            dir: self.dir.as_deref(),
            sink: Rc::new(|_, _, _| {}),
            view: self.view.clone(),
            ids: 1 << 40,
            drops: Vec::new(),
            dragging: None,
            turns: 0.0,
            measured: &measured,
            interactive: false,
        };
        let body = self.widget.as_ref().map(|w| render(&mut cx, w, Fit { w: true, h: false }));
        text_root(div(), &self.theme)
            .w(px(self.width))
            .rounded(px(10.0))
            .shadow(vec![BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0x55 as f32 / 255.0),
                offset: point(px(0.0), px(10.0)),
                blur_radius: px(24.0),
                spread_radius: px(0.0),
            }])
            .children(body)
    }
}

pub fn draggable(cx: &mut Ctx, d: &Draggable, fit: Fit, theme: Arc<Theme>) -> El {
    let child = optional(cx, d.child, fit);
    if !d.draggable() || !cx.interactive {
        return pass(cx.el(), child);
    }
    let lifted = cx.dragging.as_ref().is_some_and(|n| n.payload == d.payload);
    let width = d.width.map_or(280.0, |w| w as f32);
    let now = DragNow { payload: d.payload.clone(), targets: d.targets.clone() };
    let feedback = d.feedback.or(d.child).cloned();
    let view = cx.view.clone();
    let dir = cx.dir.map(|p| p.to_path_buf());
    let el = pass(cx.el(), child).cursor(CursorStyle::OpenHand);
    let el = if lifted { el.opacity(0.3) } else { el };
    el.on_drag(now, move |now, _, _, app| {
        view.update(app, |v, cx| {
            v.dragging = Some(now.clone());
            cx.notify();
        })
        .ok();
        let (widget, theme, dir, view) = (feedback.clone(), theme.clone(), dir.clone(), view.clone());
        app.new(|_| Preview { widget, theme, dir, width, view })
    })
}

pub fn drop_target(cx: &mut Ctx, d: &DropTarget, fit: Fit) -> El {
    let info = Rc::new(DropInfo { id: d.id.clone(), events: owned(&d.on_drop) });
    cx.drops.push(info);
    let chain: Vec<Rc<DropInfo>> = cx.drops.iter().rev().cloned().collect();
    let child = optional(cx, d.child, fit);
    cx.drops.pop();
    let p = cx.palette;
    let accent = p.accent;
    let group = format!("drop-{}", cx.next_id());
    let fits = cx.dragging.as_ref().is_some_and(|n| n.fits(&d.id));
    let mut el = pass(cx.el().relative().group(group.clone()), child);
    set_radius(el.style(), d.radius as f32);
    if let Some(c) = d.color {
        el = el.bg(super::style::argb(c));
    }
    if fits {
        el = el.border(px(1.5)).border_color(alpha(accent, 0.35));
        if let Some(hint) = &d.hint {
            let pill = div()
                .px(px(12.0))
                .py(px(6.0))
                .rounded(px(20.0))
                .bg(hsla(accent))
                .text_size(px(12.0))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(hsla(on_accent(p)))
                .child(hint.clone());
            el = el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(14.0))
                    .flex()
                    .justify_center()
                    .opacity(0.0)
                    .group_drag_over::<DragNow>(group, |s| s.opacity(1.0))
                    .child(pill),
            );
        }
    }
    let id = d.id.clone();
    let hover = d.hover_color.map(super::style::argb).unwrap_or(alpha(accent, 0.10));
    let border = hsla(accent);
    let sink = cx.sink.clone();
    let view = cx.view.clone();
    el.drag_over::<DragNow>(move |s, now, _, _| {
        if now.fits(&id) { s.border(px(2.0)).border_color(border).bg(hover) } else { s }
    })
    .on_drop::<DragNow>(move |now, window, app| {
        view.update(app, |v, cx| {
            v.dragging = None;
            cx.notify();
        })
        .ok();
        if let Some(target) = chain.iter().find(|t| now.fits(&t.id)) {
            let extra = mx_rfw::Map::from_iter([
                ("payload".to_owned(), mx_rfw::Value::from(now.payload.as_str())),
                ("to".to_owned(), mx_rfw::Value::from(target.id.as_str())),
            ]);
            super::layout::fire(&sink, &target.events, &extra, window, app);
        }
    })
}
