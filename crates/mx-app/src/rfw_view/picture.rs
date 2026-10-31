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

const BUDGET_BYTES: usize = 256 << 20;
const MAX_ENTRIES: usize = 1024;

type Key = (PathBuf, Option<SystemTime>, u64);

#[derive(Clone)]
pub struct Loaded {
    pub image: Arc<RenderImage>,
    pub size: (u32, u32),
}

impl Loaded {
    fn bytes(&self) -> usize {
        self.size.0 as usize * self.size.1 as usize * 4
    }
}

struct Entry {
    key: Key,
    loaded: Option<Loaded>,
    used: u64,
}

impl Entry {
    fn bytes(&self) -> usize {
        self.loaded.as_ref().map_or(0, Loaded::bytes)
    }
}

struct Cache {
    entries: Vec<Entry>,
    stale: Vec<Arc<RenderImage>>,
    generation: u64,
    painted: bool,
    bytes: usize,
    budget: usize,
    decodes: usize,
}

impl Default for Cache {
    fn default() -> Self {
        Cache {
            entries: Vec::new(),
            stale: Vec::new(),
            generation: 0,
            painted: false,
            bytes: 0,
            budget: BUDGET_BYTES,
            decodes: 0,
        }
    }
}

impl Cache {
    fn remove(&mut self, i: usize) {
        let old = self.entries.remove(i);
        self.bytes -= old.bytes();
        if let Some(l) = old.loaded {
            self.stale.push(l.image);
        }
    }

    fn forget(&mut self, path: &Path) {
        while let Some(i) = self.entries.iter().position(|e| e.key.0 == path) {
            self.remove(i);
        }
    }

    fn trim(&mut self) {
        while self.bytes > self.budget || self.entries.len() > MAX_ENTRIES {
            let Some(i) = self.entries.iter().position(|e| e.used < self.generation) else { break };
            self.remove(i);
        }
    }

    fn get(&mut self, path: &Path) -> Option<Loaded> {
        if std::mem::take(&mut self.painted) {
            self.generation += 1;
        }
        let Some(key) = key(path) else {
            self.forget(path);
            return None;
        };
        if let Some(i) = self.entries.iter().position(|e| e.key == key) {
            let mut hit = self.entries.remove(i);
            hit.used = self.generation;
            let loaded = hit.loaded.clone();
            self.entries.push(hit);
            return loaded;
        }
        self.forget(path);
        self.decodes += 1;
        let loaded = decode(path);
        let entry = Entry { key, loaded: loaded.clone(), used: self.generation };
        self.bytes += entry.bytes();
        self.entries.push(entry);
        self.trim();
        loaded
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
    CACHE.with(|c| c.borrow_mut().get(path))
}

pub fn frame_painted() {
    CACHE.with(|c| c.borrow_mut().painted = true);
}

pub fn cached() -> usize {
    CACHE.with(|c| c.borrow().entries.len())
}

pub fn cached_bytes() -> usize {
    CACHE.with(|c| c.borrow().bytes)
}

pub fn decoded() -> usize {
    CACHE.with(|c| c.borrow().decodes)
}

#[cfg(test)]
pub fn set_budget(bytes: usize) {
    CACHE.with(|c| c.borrow_mut().budget = bytes);
}

fn take_stale() -> Vec<Arc<RenderImage>> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.painted = true;
        std::mem::take(&mut c.stale)
    })
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

