use gpui::{
    AlignItems, FontWeight, JustifyContent, ParentElement, StatefulInteractiveElement,
    Styled, relative,
};
#[cfg(test)]
use gpui::InteractiveElement;
use mx_rfw::decoders::{
    CrossAxisAlignment, FlexFit, MainAxisAlignment, MainAxisSize, StackFit, TextAlign, TextOverflow, TextStyle,
};
use mx_rfw::widgets::core_layout::{SizedBox, Stack, Text, Wrap};
use mx_rfw::widgets::core_widgets::{Container, Flex, Positioned};
use mx_rfw::widgets::{Local, decode};
use mx_rfw::{Child, Map, Node, Widget};

use super::layout::{Ctx, El, Fit, error_text, fire, grow, owned, pass, rigid, stretch_self};
use super::style::{align_axes, argb, axis_align, decorate, hsla, pxf, set_margin, set_padding};
use super::{paint, widgets_maestria, widgets_scroll};

pub fn render(cx: &mut Ctx, w: &Widget, fit: Fit) -> El {
    match w {
        Widget::Error(e) => error_text(cx, e, hsla(cx.palette.red)),
        Widget::Node(n) => node(cx, n, fit),
    }
}

pub fn child(cx: &mut Ctx, c: Child, fit: Fit) -> El {
    match c {
        Ok(w) => render(cx, w, fit),
        Err(e) => error_text(cx, &e, hsla(cx.palette.red)),
    }
}

pub fn optional(cx: &mut Ctx, c: Option<&Widget>, fit: Fit) -> Option<El> {
    c.map(|w| render(cx, w, fit))
}

fn node(cx: &mut Ctx, n: &Node, fit: Fit) -> El {
    let turns = std::mem::replace(&mut cx.turns, 0.0);
    match decode(n) {
        Local::Column(f) => flex(cx, &f, false, fit),
        Local::Row(f) => flex(cx, &f, true, fit),
        Local::Expanded(e) | Local::Flexible(e) => {
            let el = child(cx, e.child, fit);
            grow(el, e.flex as f32)
        }
        Local::Spacer { flex } => grow(cx.el(), flex as f32),
        Local::Text(t) => text(cx, &t, turns),
        Local::Container(c) => container(cx, &c, fit),
        Local::Padding(p) => {
            let inner = optional(cx, p.base.child, fit);
            let mut el = cx.el();
            set_padding(el.style(), &p.padding);
            pass(el, inner)
        }
        Local::SizedBox(s) => sized(cx, &s, fit),
        Local::Center(c) => aligned(cx, (0.0, 0.0), c.child, fit),
        Local::Align(a) => aligned(cx, align_axes(&a.alignment), a.child, fit),
        Local::Opacity(o) => {
            let inner = optional(cx, o.base.child, fit);
            pass(cx.el().opacity(o.opacity as f32), inner)
        }
        Local::IntrinsicHeight(s) => {
            let inner = optional(cx, s.child, fit);
            pass(cx.el(), inner)
        }
        Local::Rotation(r) => {
            cx.turns = r.turns;
            let inner = optional(cx, r.base.child, fit);
            cx.turns = 0.0;
            pass(cx.el(), inner)
        }
        Local::FractionallySizedBox(f) => {
            let inner = cx.within(true, |cx| child(cx, f.child, Fit::BOTH));
            let mut el = cx.el();
            if let Some(w) = f.width_factor {
                el = el.w(relative(w as f32));
            }
            if let Some(h) = f.height_factor {
                el = el.h(relative(h as f32));
            }
            pass(el, Some(inner))
        }
        Local::GestureDetector(g) => {
            let inner = optional(cx, g.child, fit);
            let el = pass(cx.el(), inner);
            let events = owned(&g.on_tap);
            if events.is_empty() || !cx.interactive {
                return el;
            }
            let sink = cx.sink.clone();
            el.on_click(move |_, window, app| {
                app.stop_propagation();
                fire(&sink, &events, &Map::new(), window, app);
            })
        }
        Local::Stack(s) => stack(cx, &s, fit),
        Local::Positioned(p) => positioned(cx, &p),
        Local::Wrap(w) => wrap(cx, &w, fit),
        Local::ListView(l) | Local::GridView(l) => widgets_scroll::list(cx, &l, fit),
        Local::SingleChildScrollView(s) => widgets_scroll::single(cx, &s, fit),
        Local::CircularProgressIndicator(p) => paint::circular(cx, &p),
        Local::Unsupported(n) => {
            let message = format!("{} ainda não desenha", n.full_name());
            error_text(cx, &message, hsla(cx.palette.yellow))
        }
        other => widgets_maestria::render(cx, other, fit, turns),
    }
}

fn justify(m: MainAxisAlignment) -> JustifyContent {
    match m {
        MainAxisAlignment::Start => JustifyContent::FlexStart,
        MainAxisAlignment::End => JustifyContent::FlexEnd,
        MainAxisAlignment::Center => JustifyContent::Center,
        MainAxisAlignment::SpaceBetween => JustifyContent::SpaceBetween,
        MainAxisAlignment::SpaceAround => JustifyContent::SpaceAround,
        MainAxisAlignment::SpaceEvenly => JustifyContent::SpaceEvenly,
    }
}

fn cross(c: CrossAxisAlignment) -> AlignItems {
    match c {
        CrossAxisAlignment::Start => AlignItems::FlexStart,
        CrossAxisAlignment::End => AlignItems::FlexEnd,
        CrossAxisAlignment::Center => AlignItems::Center,
        CrossAxisAlignment::Stretch => AlignItems::Stretch,
        CrossAxisAlignment::Baseline => AlignItems::Baseline,
    }
}

fn flex(cx: &mut Ctx, f: &Flex, row: bool, fit: Fit) -> El {
    let mut el = cx.el().flex();
    el = if row { el.flex_row() } else { el.flex_col() };
    {
        let s = el.style();
        s.justify_content = Some(justify(f.main_axis_alignment));
        s.align_items = Some(cross(f.cross_axis_alignment));
    }
    let stretch = f.cross_axis_alignment == CrossAxisAlignment::Stretch;
    let (main, cross_fit) = if row { (fit.w, fit.h) } else { (fit.h, fit.w) };
    let child_fit = |main_fit: bool| {
        let cross_fit = cross_fit || stretch;
        if row { Fit { w: main_fit, h: cross_fit } } else { Fit { w: cross_fit, h: main_fit } }
    };
    for c in &f.children {
        let flexed = match c {
            Ok(Widget::Node(n)) => match decode(n) {
                Local::Expanded(e) => Some((e.child, e.flex, FlexFit::Tight)),
                Local::Flexible(e) => Some((e.child, e.flex, e.fit)),
                Local::Spacer { flex } => {
                    el = el.child(grow(cx.el(), flex as f32));
                    continue;
                }
                _ => None,
            },
            _ => None,
        };
        el = match flexed {
            Some((inner, flex, FlexFit::Tight)) => {
                let item = cx.within(row || cx.bounded_w, |cx| child(cx, inner, child_fit(true)));
                el.child(grow(item, flex as f32))
            }
            Some((inner, _, FlexFit::Loose)) => {
                let mut item = cx.within(row || cx.bounded_w, |cx| child(cx, inner, child_fit(true)));
                let s = item.style();
                s.flex_shrink = Some(1.0);
                s.min_size.width = Some(pxf(0.0).into());
                s.min_size.height = Some(pxf(0.0).into());
                el.child(item)
            }
            None => {
                let item = cx.within(!row && cx.bounded_w, |cx| child(cx, c.clone(), child_fit(false)));
                el.child(rigid(item))
            }
        };
    }
    if f.main_axis_size == MainAxisSize::Max && main { stretch_self(el) } else { el }
}

pub fn apply_text_style(mut el: El, s: &TextStyle) -> El {
    if let Some(c) = s.color {
        el = el.text_color(argb(c));
    }
    if let Some(size) = s.font_size {
        el = el.text_size(pxf(size));
    }
    if let Some(w) = s.font_weight {
        el = el.font_weight(FontWeight(f32::from(w.0)));
    }
    if let Some(family) = &s.font_family {
        el = el.font_family(family.clone());
    }
    if let Some(h) = s.height {
        el = el.line_height(relative(h as f32));
    }
    if s.font_style == Some(mx_rfw::decoders::FontStyle::Italic) {
        el = el.italic();
    }
    if s.decoration.underline {
        el = el.underline();
    }
    if s.decoration.line_through {
        el = el.line_through();
    }
    el
}

fn text(cx: &mut Ctx, t: &Text, turns: f64) -> El {
    let mut el = cx.el();
    if let Some(s) = &t.style {
        el = apply_text_style(el, s);
    }
    el = match t.text_align {
        Some(TextAlign::Center) => el.text_center(),
        Some(TextAlign::End | TextAlign::Right) => el.text_right(),
        _ => el,
    };
    let quarter = (turns.rem_euclid(0.5) - 0.25).abs() < 0.01;
    if quarter {
        let vertical: Vec<String> = t.text.chars().map(String::from).collect();
        return el.flex().flex_col().items_center().line_height(relative(1.0)).children(vertical);
    }
    let ellipsis = t.overflow == Some(TextOverflow::Ellipsis);
    el = match t.max_lines {
        Some(n) if n > 0 => {
            let el = el.line_clamp(n as usize).overflow_hidden();
            if ellipsis { el.text_ellipsis() } else { el }
        }
        _ => el,
    };
    #[cfg(test)]
    let el = el.debug_selector(|| format!("text:{}", t.text));
    el.child(t.text.clone())
}

fn container(cx: &mut Ctx, c: &Container, fit: Fit) -> El {
    let w_fit = fit.w || c.width.is_some();
    let h_fit = fit.h || c.height.is_some();
    let inner_fit = Fit { w: w_fit, h: h_fit };
    let bounded = cx.bounded_w || c.width.is_some() || c.constraints.is_some_and(|k| k.max_width.is_finite());
    let mut el = cx.within(bounded, |cx| match (c.alignment, c.child) {
        (Some(a), Some(w)) => aligned(cx, align_axes(&a), Some(w), inner_fit),
        (_, w) => {
            let inner = optional(cx, w, inner_fit);
            pass(cx.el(), inner)
        }
    });
    let s = el.style();
    if let Some(p) = &c.padding {
        set_padding(s, p);
    }
    if let Some(m) = &c.margin {
        set_margin(s, m);
    }
    if let Some(col) = c.color {
        s.background = Some(argb(col).into());
    }
    if let Some(d) = &c.decoration {
        decorate(s, d);
    }
    if let Some(w) = c.width {
        s.size.width = Some(pxf(w).into());
        s.flex_shrink = Some(0.0);
    }
    if let Some(h) = c.height {
        s.size.height = Some(pxf(h).into());
    }
    if let Some(k) = &c.constraints {
        constrain(s, k);
    }
    if c.clip_behavior != mx_rfw::decoders::Clip::None {
        el = el.overflow_hidden();
    }
    let fills = c.child.is_none() || c.alignment.is_some();
    if fills && fit.any() { stretch_self(el) } else { el }
}

fn sized(cx: &mut Ctx, s: &SizedBox, fit: Fit) -> El {
    let inner_fit = Fit { w: fit.w || s.width.is_some(), h: fit.h || s.height.is_some() };
    let inner = cx.within(cx.bounded_w || s.width.is_some(), |cx| optional(cx, s.child, inner_fit));
    let mut el = pass(cx.el(), inner);
    if let Some(w) = s.width {
        el = el.w(pxf(w)).flex_none();
    }
    if let Some(h) = s.height {
        el = el.h(pxf(h)).flex_none();
    }
    el
}

pub fn aligned(cx: &mut Ctx, (x, y): (f64, f64), w: Option<&Widget>, fit: Fit) -> El {
    let inner = optional(cx, w, Fit { w: false, h: false });
    let mut el = cx.el().flex().flex_col();
    {
        let s = el.style();
        s.justify_content = Some(match axis_align(y) {
            AlignItems::FlexStart => JustifyContent::FlexStart,
            AlignItems::FlexEnd => JustifyContent::FlexEnd,
            _ => JustifyContent::Center,
        });
        s.align_items = Some(axis_align(x));
    }
    if let Some(i) = inner {
        el = el.child(i);
    }
    if fit.any() { stretch_self(el) } else { el }
}

fn stack(cx: &mut Ctx, s: &Stack, fit: Fit) -> El {
    let expand = s.fit == StackFit::Expand;
    let mut el = cx.el().relative().flex().flex_col();
    el.style().align_items = Some(if expand { AlignItems::Stretch } else { AlignItems::FlexStart });
    let mut in_flow = false;
    for c in &s.children {
        let is_positioned = matches!(c, Ok(Widget::Node(n)) if n.name == "Positioned");
        let item = child(cx, c.clone(), if expand { fit } else { Fit { w: false, h: false } });
        el = el.child(if is_positioned {
            item
        } else if !in_flow {
            in_flow = true;
            if expand { grow(item, 1.0) } else { item }
        } else if expand {
            item.absolute().top_0().left_0().size_full()
        } else {
            item.absolute().top_0().left_0()
        });
    }
    if (expand || !in_flow) && fit.any() { stretch_self(el) } else { el }
}

fn positioned(cx: &mut Ctx, p: &Positioned) -> El {
    let fit = Fit {
        w: p.width.is_some() || (p.start.is_some() && p.end.is_some()),
        h: p.height.is_some() || (p.top.is_some() && p.bottom.is_some()),
    };
    let mut el = cx.within(fit.w, |cx| {
        let inner = child(cx, p.child.clone(), fit);
        pass(cx.el().absolute(), Some(inner))
    });
    let s = el.style();
    let at = |v: Option<f64>| v.map(|v| pxf(v).into());
    s.inset.left = at(p.start);
    s.inset.top = at(p.top);
    s.inset.right = at(p.end);
    s.inset.bottom = at(p.bottom);
    if let Some(w) = p.width {
        s.size.width = Some(pxf(w).into());
    }
    if let Some(h) = p.height {
        s.size.height = Some(pxf(h).into());
    }
    el
}

fn wrap(cx: &mut Ctx, w: &Wrap, fit: Fit) -> El {
    use mx_rfw::decoders::{WrapAlignment, WrapCrossAlignment};
    let mut el = cx.el().flex().flex_row().flex_wrap().gap_x(pxf(w.spacing)).gap_y(pxf(w.run_spacing));
    let s = el.style();
    s.align_items = Some(match w.cross_axis_alignment {
        WrapCrossAlignment::Start => AlignItems::FlexStart,
        WrapCrossAlignment::End => AlignItems::FlexEnd,
        WrapCrossAlignment::Center => AlignItems::Center,
    });
    s.justify_content = Some(match w.alignment {
        WrapAlignment::Start => JustifyContent::FlexStart,
        WrapAlignment::End => JustifyContent::FlexEnd,
        WrapAlignment::Center => JustifyContent::Center,
        WrapAlignment::SpaceBetween => JustifyContent::SpaceBetween,
        WrapAlignment::SpaceAround => JustifyContent::SpaceAround,
        WrapAlignment::SpaceEvenly => JustifyContent::SpaceEvenly,
    });
    for c in &w.children {
        let item = child(cx, c.clone(), Fit { w: fit.w, h: false });
        el = el.child(rigid(item));
    }
    el
}

fn constrain(s: &mut gpui::StyleRefinement, k: &mx_rfw::decoders::BoxConstraints) {
    let finite = |v: f64| (v.is_finite() && v > 0.0).then(|| pxf(v).into());
    s.min_size.width = finite(k.min_width).or(s.min_size.width);
    s.min_size.height = finite(k.min_height).or(s.min_size.height);
    s.max_size.width = finite(k.max_width).or(s.max_size.width);
    s.max_size.height = finite(k.max_height).or(s.max_size.height);
}
