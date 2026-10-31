use std::path::{Path, PathBuf};
use std::time::Duration;

use super::text_input::{number, rounded};
use crate::tree::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    pub sheet: Option<String>,
    pub columns: i64,
    pub rows: i64,
    pub row: i64,
    pub start: i64,
    pub frames: i64,
    pub fps: f64,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub flip: bool,
    pub paused: bool,
    pub looping: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub row: i64,
    pub start: i64,
    pub frames: i64,
    pub fps: f64,
    pub paused: bool,
    pub looping: bool,
}

impl Sprite {
    pub fn from_node(n: &Node) -> Self {
        Self {
            sheet: n.arg("sheet").as_str().map(str::to_owned),
            columns: rounded(n.arg("columns")).unwrap_or(1),
            rows: rounded(n.arg("rows")).unwrap_or(1),
            row: rounded(n.arg("row")).unwrap_or(0),
            start: rounded(n.arg("start")).unwrap_or(0),
            frames: rounded(n.arg("frames")).unwrap_or(1),
            fps: number(n.arg("fps")).unwrap_or(6.0),
            width: number(n.arg("width")),
            height: number(n.arg("height")),
            flip: n.arg("flip").as_bool() == Some(true),
            paused: n.arg("paused").as_bool() == Some(true),
            looping: n.arg("loop").as_bool().unwrap_or(true),
        }
    }

    pub fn motion(&self) -> Motion {
        Motion {
            row: self.row,
            start: self.start,
            frames: self.frames,
            fps: self.fps,
            paused: self.paused,
            looping: self.looping,
        }
    }

    pub fn animates(&self) -> bool {
        !self.paused && self.frames > 1 && self.fps > 0.0
    }

    pub fn frame_at(&self, elapsed: Duration) -> i64 {
        if !self.animates() {
            return 0;
        }
        sprite_frame(elapsed, self.fps, self.frames, self.looping)
    }

    pub fn cell(&self, frame: i64) -> (i64, i64) {
        let cols = self.columns.max(1);
        let rows = self.rows.max(1);
        ((self.start + frame).clamp(0, cols - 1), self.row.clamp(0, rows - 1))
    }

    pub fn grid(&self) -> (i64, i64) {
        (self.columns.max(1), self.rows.max(1))
    }

    pub fn finished(&self, frame: i64) -> bool {
        !self.looping && frame == self.frames - 1
    }
}

pub fn sprite_frame(elapsed: Duration, fps: f64, frames: i64, looping: bool) -> i64 {
    if frames <= 1 || fps <= 0.0 {
        return 0;
    }
    let n = (elapsed.as_micros() as f64 * fps / 1e6).floor() as i64;
    if looping { n.rem_euclid(frames) } else { n.clamp(0, frames - 1) }
}

fn absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf())
    }
}

pub fn plugin_file_of(rel: Option<&str>, dir: Option<&Path>) -> Option<PathBuf> {
    let rel = rel.filter(|r| !r.is_empty() && !r.contains(".."))?;
    let root = absolute(dir?);
    let root_text = root.to_string_lossy().trim_end_matches('/').to_owned();
    let file = PathBuf::from(format!("{root_text}/{rel}"));
    if !file.to_string_lossy().starts_with(&format!("{root_text}/")) {
        return None;
    }
    file.is_file().then_some(file)
}

pub fn plugin_or_data_file_of(rel: Option<&str>, plugin_dir: Option<&Path>, data_dir: Option<&Path>) -> Option<PathBuf> {
    plugin_file_of(rel, plugin_dir).or_else(|| plugin_file_of(rel, data_dir))
}
