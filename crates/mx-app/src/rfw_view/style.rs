use gpui::{
    AbsoluteLength, AlignItems, BorderStyle, Corners, DefiniteLength, Edges, Hsla, Length,
    Pixels, Rgba, StyleRefinement, px,
};
use mx_rfw::decoders::{Alignment, BorderRadius, BoxDecoration, BoxShape, Color as RfwColor, EdgeInsets};
use mx_rfw::{Map, Value};
use mx_theme::{Color, Palette, Typography};

pub const UI_FONT: &str = "Roboto";
pub const UI_LINE: f32 = 1.17;

pub fn hsla(c: Color) -> Hsla {
    Rgba {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: f32::from(c.a) / 255.0,
    }
    .into()
}

pub fn argb(c: RfwColor) -> Hsla {
    hsla(Color::from_argb(c.0))
}

pub fn alpha(c: Color, a: f32) -> Hsla {
    let mut h = hsla(c);
    h.a *= a;
    h
}

pub fn hex(c: Hsla) -> String {
    let rgba: Rgba = c.into();
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", ch(rgba.r), ch(rgba.g), ch(rgba.b))
}

pub fn opacity_of(c: Hsla) -> f32 {
    c.a
}

pub fn accent_is_dark(p: &Palette) -> bool {
    let l = p.accent.luminance();
    (l + 0.05) * (l + 0.05) <= 0.15
}

pub fn on_accent(p: &Palette) -> Color {
    if accent_is_dark(p) { Color::WHITE } else { p.canvas }
}

pub fn rfw_theme(p: &Palette, typography: &Typography) -> Value {
    let mut m = Map::new();
    m.insert("dark".into(), Value::Bool(p.dark));
    let colors = [
        ("canvas", p.canvas),
        ("bg", p.bg),
        ("sidebar", p.bg_sidebar),
        ("hover", p.bg_hover),
        ("active", p.bg_active),
        ("border", p.border),
        ("fg", p.fg),
        ("dim", p.fg_dim),
        ("faint", p.fg_faint),
        ("accent", p.accent),
        ("onAccent", on_accent(p)),
        ("green", p.green),
        ("yellow", p.yellow),
        ("red", p.red),
        ("purple", p.purple),
        ("blue", p.ansi.blue),
        ("cyan", p.ansi.cyan),
        ("magenta", p.ansi.magenta),
    ];
    for (k, c) in colors {
        m.insert(k.into(), Value::Int(i64::from(c.to_argb())));
    }
    m.insert("mono".into(), Value::from(typography.mono()));
    Value::Map(m)
}

pub fn edges(e: &EdgeInsets) -> Edges<DefiniteLength> {
    Edges {
        top: px(e.top as f32).into(),
        right: px(e.end as f32).into(),
        bottom: px(e.bottom as f32).into(),
        left: px(e.start as f32).into(),
    }
}

pub fn set_padding(s: &mut StyleRefinement, e: &EdgeInsets) {
    let e = edges(e);
    s.padding.top = Some(e.top);
    s.padding.right = Some(e.right);
    s.padding.bottom = Some(e.bottom);
    s.padding.left = Some(e.left);
}

pub fn set_margin(s: &mut StyleRefinement, e: &EdgeInsets) {
    let l = |v: f64| Length::Definite(px(v as f32).into());
    s.margin.top = Some(l(e.top));
    s.margin.right = Some(l(e.end));
    s.margin.bottom = Some(l(e.bottom));
    s.margin.left = Some(l(e.start));
}

pub fn radii(r: &BorderRadius) -> Corners<AbsoluteLength> {
    let a = |v: f64| AbsoluteLength::Pixels(px(v as f32));
    Corners {
        top_left: a(r.top_start.x),
        top_right: a(r.top_end.x),
        bottom_left: a(r.bottom_start.x),
        bottom_right: a(r.bottom_end.x),
    }
}

pub fn set_radius(s: &mut StyleRefinement, r: f32) {
    let a = Some(AbsoluteLength::Pixels(px(r)));
    s.corner_radii.top_left = a;
    s.corner_radii.top_right = a;
    s.corner_radii.bottom_left = a;
    s.corner_radii.bottom_right = a;
}

pub fn set_border(s: &mut StyleRefinement, color: Hsla, width: [f32; 4]) {
    let w = |v: f32| Some(AbsoluteLength::Pixels(px(v)));
    s.border_widths.top = w(width[0]);
    s.border_widths.right = w(width[1]);
    s.border_widths.bottom = w(width[2]);
    s.border_widths.left = w(width[3]);
    s.border_color = Some(color);
    s.border_style = Some(BorderStyle::Solid);
}

pub fn decorate(s: &mut StyleRefinement, d: &BoxDecoration) {
    if let Some(c) = d.color {
        s.background = Some(argb(c).into());
    }
    if d.shape == BoxShape::Circle {
        set_radius(s, 9999.0);
    } else if let Some(r) = &d.border_radius {
        let c = radii(r);
        s.corner_radii.top_left = Some(c.top_left);
        s.corner_radii.top_right = Some(c.top_right);
        s.corner_radii.bottom_left = Some(c.bottom_left);
        s.corner_radii.bottom_right = Some(c.bottom_right);
    }
    if let Some(b) = &d.border {
        let sides = [&b.top, &b.end, &b.bottom, &b.start];
        let live = |i: usize| sides[i].style != mx_rfw::decoders::BorderStyle::None;
        let width = std::array::from_fn(|i| if live(i) { sides[i].width as f32 } else { 0.0 });
        if let Some(side) = (0..4).find(|&i| live(i)) {
            set_border(s, argb(sides[side].color), width);
        }
    }
}

pub fn align_axes(a: &Alignment) -> (f64, f64) {
    match *a {
        Alignment::Absolute { x, y } => (x, y),
        Alignment::Directional { start, y } => (start, y),
    }
}

pub fn axis_align(v: f64) -> AlignItems {
    if v < -0.33 {
        AlignItems::FlexStart
    } else if v > 0.33 {
        AlignItems::FlexEnd
    } else {
        AlignItems::Center
    }
}

pub fn pxf(v: f64) -> Pixels {
    px(v as f32)
}
