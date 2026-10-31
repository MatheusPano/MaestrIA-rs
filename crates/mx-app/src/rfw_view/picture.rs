use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use gpui::{Bounds, Corners, ParentElement, Pixels, RenderImage, Styled, Window, canvas, point, px, size};
use mx_rfw::widgets::picture::Picture;
use mx_rfw::widgets::sprite::plugin_or_data_file_of;
use mx_rfw::widgets::zoom_view::{BoxFit, fitted_scale};

use super::layout::{Ctx, El};
use super::style::pxf;

const MAX_PICTURES: usize = 64;

type Key = (PathBuf, Option<SystemTime>, u64);

#[derive(Clone)]
pub struct Loaded {
    pub image: Arc<RenderImage>,
    pub size: (u32, u32),
}

#[derive(Default)]
struct Cache {
    entries: Vec<(Key, Option<Loaded>)>,
    stale: Vec<Arc<RenderImage>>,
}

impl Cache {
    fn retire(&mut self, entry: (Key, Option<Loaded>)) {
        if let Some(l) = entry.1 {
            self.stale.push(l.image);
        }
    }
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::default();
}

fn key(path: &Path) -> Option<Key> {
    let meta = std::fs::metadata(path).ok().filter(std::fs::Metadata::is_file)?;
    Some((path.to_path_buf(), meta.modified().ok(), meta.len()))
}

fn decode(path: &Path) -> Option<Loaded> {
    let mut img = image::open(path).ok()?.into_rgba8();
    for px in img.pixels_mut() {
        px.0.swap(0, 2);
    }
    let size = img.dimensions();
    Some(Loaded { image: Arc::new(RenderImage::new(vec![image::Frame::new(img)])), size })
}

pub fn load(path: &Path) -> Option<Loaded> {
    let key = key(path)?;
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(i) = c.entries.iter().position(|(k, _)| *k == key) {
            let hit = c.entries.remove(i);
            let loaded = hit.1.clone();
            c.entries.push(hit);
            return loaded;
        }
        let loaded = decode(path);
        while let Some(i) = c.entries.iter().position(|(k, _)| k.0 == key.0) {
            let old = c.entries.remove(i);
            c.retire(old);
        }
        while c.entries.len() >= MAX_PICTURES {
            let old = c.entries.remove(0);
            c.retire(old);
        }
        c.entries.push((key, loaded.clone()));
        loaded
    })
}

pub fn cached() -> usize {
    CACHE.with(|c| c.borrow().entries.len())
}

fn take_stale() -> Vec<Arc<RenderImage>> {
    CACHE.with(|c| std::mem::take(&mut c.borrow_mut().stale))
}

pub fn box_size(width: Option<f64>, height: Option<f64>, natural: Option<(f64, f64)>) -> (f64, f64) {
    let (nw, nh) = natural.filter(|n| n.0 > 0.0 && n.1 > 0.0).unwrap_or((0.0, 0.0));
    match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) if nw > 0.0 => (w, w * nh / nw),
        (None, Some(h)) if nh > 0.0 => (h * nw / nh, h),
        (w, h) => (w.unwrap_or(nw), h.unwrap_or(nh)),
    }
}

pub fn placed(fit: BoxFit, natural: (f64, f64), outer: (f64, f64)) -> (f64, f64, f64, f64) {
    if fit == BoxFit::Fill {
        return (0.0, 0.0, outer.0, outer.1);
    }
    let k = fitted_scale(fit, natural, outer);
    let (w, h) = (natural.0 * k, natural.1 * k);
    ((outer.0 - w) / 2.0, (outer.1 - h) / 2.0, w, h)
}

pub fn render(cx: &mut Ctx, p: &Picture) -> El {
    let loaded = plugin_or_data_file_of(p.path.as_deref(), cx.dir, cx.data_dir).and_then(|f| load(&f));
    let natural = loaded.as_ref().map(|l| (f64::from(l.size.0), f64::from(l.size.1)));
    let (w, h) = box_size(p.width, p.height, natural);
    let el = cx.el().flex_none().w(pxf(w)).h(pxf(h)).overflow_hidden();
    let (Some(loaded), Some(natural)) = (loaded, natural) else { return el };
    let fit = p.fit;
    el.child(
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window: &mut Window, _| {
                for old in take_stale() {
                    window.drop_image(old).ok();
                }
                let outer = (f64::from(f32::from(bounds.size.width)), f64::from(f32::from(bounds.size.height)));
                let (x, y, w, h) = placed(fit, natural, outer);
                let at = Bounds {
                    origin: bounds.origin + point(px(x as f32), px(y as f32)),
                    size: size(px(w as f32), px(h as f32)),
                };
                window.paint_image(at, Corners::default(), loaded.image.clone(), 0, false).ok();
            },
        )
        .size_full(),
    )
}
