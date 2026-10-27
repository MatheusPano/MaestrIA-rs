use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use gpui::{App, Bounds, Corners, ParentElement, Pixels, RenderImage, Styled, Task, WeakEntity, Window, canvas};
use image::RgbaImage;
use mx_rfw::widgets::sprite::{Motion, Sprite, plugin_file_of};

use super::layout::{Ctx, El};
use super::style::pxf;
use super::view::RfwView;

const MAX_CELLS: usize = 4096;

type SheetKey = (PathBuf, Option<SystemTime>);
type CellKey = (PathBuf, Option<SystemTime>, u32, u32, u32, u32, bool, u32);

thread_local! {
    static SHEETS: RefCell<HashMap<SheetKey, Option<Arc<RgbaImage>>>> = RefCell::new(HashMap::new());
    static CELLS: RefCell<HashMap<CellKey, Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn sheet(path: &Path) -> Option<Arc<RgbaImage>> {
    let key = (path.to_path_buf(), modified(path));
    SHEETS.with(|s| {
        s.borrow_mut()
            .entry(key)
            .or_insert_with(|| image::open(path).ok().map(|i| Arc::new(i.into_rgba8())))
            .clone()
    })
}

pub fn cell_pixels(sheet: &RgbaImage, cell: (u32, u32, u32, u32), flip: bool, k: u32) -> RgbaImage {
    let (x0, y0, w, h) = cell;
    let k = k.max(1);
    let mut out = RgbaImage::new((w * k).max(1), (h * k).max(1));
    for (x, y, px) in out.enumerate_pixels_mut() {
        let sx = x / k;
        let sx = if flip { w.saturating_sub(1) - sx.min(w.saturating_sub(1)) } else { sx };
        let (gx, gy) = (x0 + sx, y0 + y / k);
        if gx < sheet.width() && gy < sheet.height() {
            let p = sheet.get_pixel(gx, gy).0;
            *px = image::Rgba([p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

fn cell_image(path: &Path, s: &Sprite, frame: i64, target: (f32, f32)) -> Option<(Arc<RenderImage>, (f32, f32))> {
    let img = sheet(path)?;
    let (cols, rows) = s.grid();
    let (cw, ch) = (img.width() as f32 / cols as f32, img.height() as f32 / rows as f32);
    let (col, row) = s.cell(frame);
    let cell = ((col as f32 * cw) as u32, (row as f32 * ch) as u32, cw.max(1.0) as u32, ch.max(1.0) as u32);
    let k = ((target.0 / cw.max(1.0)).max(target.1 / ch.max(1.0)).ceil() as u32).clamp(1, 32);
    let key = (path.to_path_buf(), modified(path), cell.0, cell.1, cell.2, cell.3, s.flip, k);
    let made = CELLS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > MAX_CELLS {
            c.clear();
        }
        c.entry(key)
            .or_insert_with(|| Arc::new(RenderImage::new(vec![image::Frame::new(cell_pixels(&img, cell, s.flip, k))])))
            .clone()
    });
    Some((made, (cw, ch)))
}

pub fn natural_size(path: Option<&Path>, s: &Sprite) -> Option<(f32, f32)> {
    let img = sheet(path?)?;
    let (cols, rows) = s.grid();
    Some((img.width() as f32 / cols as f32, img.height() as f32 / rows as f32))
}

#[derive(Default)]
pub struct SpriteState {
    motion: Option<Motion>,
    started: Option<Instant>,
    stopped_at: Option<i64>,
    timer: Option<(Instant, Task<()>)>,
}

impl SpriteState {
    pub fn frame(&mut self, s: &Sprite, now: Instant) -> i64 {
        if self.motion != Some(s.motion()) {
            self.motion = Some(s.motion());
            self.started = Some(now);
            self.stopped_at = None;
        }
        if let Some(f) = self.stopped_at {
            return f;
        }
        let elapsed = now.duration_since(self.started.unwrap_or(now));
        let f = s.frame_at(elapsed);
        if s.finished(f) {
            self.stopped_at = Some(f);
        }
        f
    }

    pub fn next_change(&self, s: &Sprite, now: Instant) -> Option<Instant> {
        if !s.animates() || self.stopped_at.is_some() {
            return None;
        }
        let start = self.started?;
        let step = 1e6 / s.fps;
        let elapsed = now.duration_since(start).as_micros() as f64;
        let next = ((elapsed / step).floor() + 1.0) * step;
        Some(start + Duration::from_micros(next.ceil() as u64))
    }
}

pub type SpriteSlot = Rc<RefCell<SpriteState>>;

fn schedule(slot: &SpriteSlot, at: Instant, view: WeakEntity<RfwView>, cx: &mut App) {
    let mut st = slot.borrow_mut();
    if st.timer.as_ref().is_some_and(|(t, _)| *t == at) {
        return;
    }
    let wait = at.saturating_duration_since(Instant::now());
    let task = cx.spawn(async move |cx| {
        cx.background_executor().timer(wait).await;
        view.update(cx, |_, cx| cx.notify()).ok();
    });
    st.timer = Some((at, task));
}

pub fn render(cx: &mut Ctx, s: &Sprite) -> El {
    let file = plugin_file_of(s.sheet.as_deref(), cx.dir);
    let natural = natural_size(file.as_deref(), s);
    let width = s.width.map(|w| w as f32).or(natural.map(|n| n.0)).unwrap_or(0.0);
    let height = s.height.map(|h| h as f32).or(natural.map(|n| n.1)).unwrap_or(0.0);
    let el = cx.el().flex_none().w(pxf(width as f64)).h(pxf(height as f64));
    let (Some(path), Some(states)) = (file, cx.widgets) else { return el };
    let slot = states.sprites.next();
    let now = Instant::now();
    let frame = slot.borrow_mut().frame(s, now);
    let next = slot.borrow().next_change(s, now);
    let view = cx.view.clone();
    let spec = s.clone();
    el.child(
        canvas(
            move |_, _, cx: &mut App| {
                if let Some(at) = next {
                    schedule(&slot, at, view, cx);
                }
            },
            move |bounds: Bounds<Pixels>, _, window: &mut Window, _| {
                let scale = window.scale_factor();
                let target = (f32::from(bounds.size.width) * scale, f32::from(bounds.size.height) * scale);
                if let Some((image, _)) = cell_image(&path, &spec, frame, target) {
                    window.paint_image(bounds, Corners::default(), image, 0, false).ok();
                }
            },
        )
        .size_full(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(frames: i64, fps: f64) -> Sprite {
        Sprite {
            sheet: None,
            columns: 4,
            rows: 2,
            row: 0,
            start: 0,
            frames,
            fps,
            width: Some(16.0),
            height: Some(24.0),
            flip: false,
            paused: false,
            looping: true,
        }
    }

    #[test]
    fn sp5_the_same_motion_keeps_its_frame_and_a_new_one_restarts() {
        let t0 = Instant::now();
        let mut st = SpriteState::default();
        let s = spec(4, 10.0);
        assert_eq!(st.frame(&s, t0), 0);
        let later = t0 + Duration::from_millis(260);
        assert_eq!(st.frame(&s, later), 2);
        assert_eq!(st.frame(&s.clone(), later), 2);
        let row1 = Sprite { row: 1, ..s.clone() };
        assert_eq!(st.frame(&row1, later), 0);
        let paused = Sprite { paused: true, ..row1 };
        assert_eq!(st.frame(&paused, later), 0);
        assert_eq!(st.frame(&paused, later + Duration::from_millis(500)), 0);
        assert_eq!(st.next_change(&paused, later), None);
    }

    #[test]
    fn the_next_change_is_the_next_frame_boundary_and_a_finished_sprite_stops() {
        let t0 = Instant::now();
        let mut st = SpriteState::default();
        let s = spec(3, 4.0);
        st.frame(&s, t0);
        let next = st.next_change(&s, t0 + Duration::from_millis(100)).unwrap();
        assert_eq!(next.duration_since(t0), Duration::from_millis(250));
        let once = Sprite { looping: false, ..s };
        let mut st = SpriteState::default();
        st.frame(&once, t0);
        assert_eq!(st.frame(&once, t0 + Duration::from_secs(9)), 2);
        assert_eq!(st.next_change(&once, t0 + Duration::from_secs(9)), None);
    }

    #[test]
    fn cells_are_cut_scaled_by_nearest_neighbour_and_flipped() {
        let mut sheet = RgbaImage::new(4, 1);
        for x in 0..4 {
            sheet.put_pixel(x, 0, image::Rgba([x as u8 * 10, 0, 0, 255]));
        }
        let cell = cell_pixels(&sheet, (2, 0, 2, 1), false, 2);
        assert_eq!(cell.dimensions(), (4, 2));
        assert_eq!(cell.get_pixel(0, 1).0, [0, 0, 20, 255]);
        assert_eq!(cell.get_pixel(3, 0).0, [0, 0, 30, 255]);
        let flipped = cell_pixels(&sheet, (2, 0, 2, 1), true, 1);
        assert_eq!(flipped.get_pixel(0, 0).0, [0, 0, 30, 255]);
    }
}
