use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use gpui::{Hsla, IntoElement, ObjectFit, RenderImage, Styled, StyledImage, div, img, prelude::*, px};
use mx_plugins::ui::icons::{GENERIC_ICON, material_icon, svg_of};

use super::icon_paths::ICON_PATHS;
use super::style::{hex, opacity_of};

thread_local! {
    static IMAGES: RefCell<HashMap<String, Option<Arc<RenderImage>>>> = RefCell::new(HashMap::new());
    static FILES: RefCell<HashMap<String, Option<String>>> = RefCell::new(HashMap::new());
}

const SCALE: f32 = 2.0;

static FONT_DB: std::sync::LazyLock<Arc<resvg::usvg::fontdb::Database>> = std::sync::LazyLock::new(|| {
    let mut db = resvg::usvg::fontdb::Database::new();
    db.load_system_fonts();
    Arc::new(db)
});

pub fn rasterize(svg: &str) -> Option<Arc<RenderImage>> {
    let mut opts = resvg::usvg::Options::default();
    if svg.contains("<text") {
        opts.fontdb = FONT_DB.clone();
    }
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &opts).ok()?;
    let size = tree.size();
    let (w, h) = ((size.width() * SCALE).ceil() as u32, (size.height() * SCALE).ceil() as u32);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(SCALE, SCALE), &mut pixmap.as_mut());
    let mut data = pixmap.take();
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2);
        if px[3] > 0 {
            let a = f32::from(px[3]) / 255.0;
            for c in &mut px[..3] {
                *c = (f32::from(*c) / a).min(255.0) as u8;
            }
        }
    }
    let buffer = image::ImageBuffer::from_raw(w.max(1), h.max(1), data)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

fn cached(key: String, make: impl FnOnce() -> String) -> Option<Arc<RenderImage>> {
    IMAGES.with(|m| m.borrow_mut().entry(key).or_insert_with(|| rasterize(&make())).clone())
}

pub fn material_path(name: &str) -> Option<&'static str> {
    ICON_PATHS.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}

pub fn plugin_icon(name: Option<&str>) -> Option<&'static str> {
    name.and_then(material_icon)
}

fn deg(turns: f64) -> f64 {
    turns * 360.0
}

pub fn material_image(name: &str, size: f32, color: Hsla, turns: f64) -> Option<Arc<RenderImage>> {
    let d = material_path(name)?;
    let key = format!("m:{name}:{size}:{}:{}:{turns}", hex(color), opacity_of(color));
    cached(key, || {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 512 512\">\
             <g transform=\"rotate({} 256 256)\"><path fill=\"{}\" fill-opacity=\"{}\" d=\"{d}\"/></g></svg>",
            deg(turns),
            hex(color),
            opacity_of(color)
        )
    })
}

fn read(file: &Path) -> Option<String> {
    let key = file.to_string_lossy().into_owned();
    FILES.with(|m| m.borrow_mut().entry(key).or_insert_with(|| std::fs::read_to_string(file).ok()).clone())
}

pub fn sized_root(svg: &str, size: f32) -> Option<String> {
    let start = svg.find("<svg")?;
    let end = start + svg[start..].find('>')?;
    let tag = &svg[start..end];
    let strip = regex::Regex::new(r#"\s(width|height)="[^"]*""#).ok()?;
    let tag = strip.replace_all(tag, "");
    Some(format!("{tag} width=\"{size}\" height=\"{size}\"{}", &svg[end..]))
}

pub fn tinted_file(file: &Path, size: f32, color: Hsla, turns: f64) -> Option<Arc<RenderImage>> {
    let key = format!("f:{}:{size}:{}:{}:{turns}", file.display(), hex(color), opacity_of(color));
    let inner = sized_root(&read(file)?, size)?;
    cached(key, || {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 {size} {size}\">\
             <defs><filter id=\"t\" x=\"-0.5\" y=\"-0.5\" width=\"2\" height=\"2\">\
             <feFlood flood-color=\"{}\" flood-opacity=\"{}\"/><feComposite in2=\"SourceAlpha\" operator=\"in\"/></filter></defs>\
             <g filter=\"url(#t)\"><g transform=\"rotate({} {} {})\">{inner}</g></g></svg>",
            hex(color),
            opacity_of(color),
            deg(turns),
            size / 2.0,
            size / 2.0
        )
    })
}

pub fn raw_svg(svg: &str) -> Option<Arc<RenderImage>> {
    let key = format!("r:{svg}");
    cached(key, || svg.to_owned())
}

pub fn svg_element(svg: &str, w: Option<f32>, h: Option<f32>) -> gpui::Div {
    let mut frame = div().flex_none();
    if let Some(w) = w {
        frame = frame.w(px(w));
    }
    if let Some(h) = h {
        frame = frame.h(px(h));
    }
    let Some(image) = raw_svg(svg) else { return frame };
    let natural = image.size(0);
    let (nw, nh) = (natural.width.0 as f32 / SCALE, natural.height.0 as f32 / SCALE);
    frame = frame.w(px(w.unwrap_or(nw))).h(px(h.unwrap_or(nh)));
    frame.child(img(image).size_full())
}

pub fn named(name: &str, size: f32, color: Hsla) -> gpui::AnyElement {
    named_turned(name, size, color, 0.0)
}

pub fn named_turned(name: &str, size: f32, color: Hsla, turns: f64) -> gpui::AnyElement {
    let frame = div().flex_none().size(px(size));
    match material_image(name, size, color, turns) {
        Some(image) => frame.child(img(image).size_full().object_fit(ObjectFit::Contain)).into_any_element(),
        None => frame.into_any_element(),
    }
}

pub fn glyph(icon: Option<&str>, dir: Option<&Path>, size: f32, color: Hsla, turns: f64) -> gpui::AnyElement {
    let file = icon.zip(dir).and_then(|(i, d)| svg_of(i, d));
    if let Some(image) = file.and_then(|f| tinted_file(&f, size, color, turns)) {
        return div().flex_none().size(px(size)).child(img(image).size_full()).into_any_element();
    }
    named_turned(plugin_icon(icon).unwrap_or(GENERIC_ICON), size, color, turns)
}
