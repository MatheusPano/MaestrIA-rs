use serde_json::{Map, Value};

use crate::rpc::FloatContent;
use crate::ui::view::PluginView;

pub const MIN_WIDTH: f64 = 80.0;
pub const MAX_WIDTH: f64 = 480.0;
pub const MIN_HEIGHT: f64 = 32.0;
pub const MAX_HEIGHT: f64 = 320.0;
pub const SNAP: f64 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl FloatCorner {
    pub const ALL: [FloatCorner; 4] = [Self::TopLeft, Self::TopRight, Self::BottomLeft, Self::BottomRight];

    pub fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "topLeft",
            Self::TopRight => "topRight",
            Self::BottomLeft => "bottomLeft",
            Self::BottomRight => "bottomRight",
        }
    }

    pub fn parse(raw: &Value) -> Option<Self> {
        let name = raw.as_str()?;
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    pub fn right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
    }

    pub fn bottom(self) -> bool {
        matches!(self, Self::BottomLeft | Self::BottomRight)
    }

    pub fn of(right: bool, bottom: bool) -> Self {
        match (right, bottom) {
            (false, false) => Self::TopLeft,
            (true, false) => Self::TopRight,
            (false, true) => Self::BottomLeft,
            (true, true) => Self::BottomRight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatSpot {
    pub corner: FloatCorner,
    pub dx: f64,
    pub dy: f64,
}

impl FloatSpot {
    pub fn origin(&self, w: f64, h: f64, window_w: f64, window_h: f64, margin: f64) -> (f64, f64) {
        let x = if self.corner.right() { window_w - w - self.dx } else { self.dx };
        let y = if self.corner.bottom() { window_h - h - self.dy } else { self.dy };
        let max_x = (window_w - w - margin).max(margin);
        let max_y = (window_h - h - margin).max(margin);
        (x.clamp(margin, max_x), y.clamp(margin, max_y))
    }

    pub fn dropped(x: f64, y: f64, w: f64, h: f64, window_w: f64, window_h: f64, margin: f64) -> FloatSpot {
        let right = x + w / 2.0 > window_w / 2.0;
        let bottom = y + h / 2.0 > window_h / 2.0;
        let snap = |d: f64| if d < margin + SNAP { margin } else { d };
        let dx = if right { window_w - x - w } else { x };
        let dy = if bottom { window_h - y - h } else { y };
        FloatSpot { corner: FloatCorner::of(right, bottom), dx: snap(dx), dy: snap(dy) }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginFloat {
    pub tab_id: String,
    pub view: PluginView,
    pub width: f64,
    pub height: f64,
    pub start: FloatCorner,
}

impl PluginFloat {
    pub fn key(&self) -> String {
        format!("{}/{}", self.view.plugin_id, self.view.id)
    }

    fn apply(&mut self, c: &FloatContent) {
        if let Some(w) = c.width {
            self.width = w.clamp(MIN_WIDTH, MAX_WIDTH);
        }
        if let Some(h) = c.height {
            self.height = h.clamp(MIN_HEIGHT, MAX_HEIGHT);
        }
        if let Some(r) = &c.rfw {
            self.view.set_rfw(r);
        }
        if let Some(b) = &c.blocks {
            self.view.rfw_library = None;
            self.view.set_blocks(b.clone());
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PluginFloats {
    shown: Vec<PluginFloat>,
    pub spots: Vec<(String, FloatSpot)>,
    pub revision: u64,
}

impl PluginFloats {
    pub fn shown(&self) -> &[PluginFloat] {
        &self.shown
    }

    pub fn of(&self, plugin_id: &str, id: &str) -> Option<&PluginFloat> {
        let key = format!("{plugin_id}/{id}");
        self.shown.iter().find(|f| f.key() == key)
    }

    pub fn of_mut(&mut self, plugin_id: &str, id: &str) -> Option<&mut PluginFloat> {
        let key = format!("{plugin_id}/{id}");
        self.shown.iter_mut().find(|f| f.key() == key)
    }

    pub fn show(&mut self, plugin_id: &str, plugin_name: &str, c: &FloatContent) -> &PluginFloat {
        let at = match self.shown.iter().position(|f| f.view.plugin_id == plugin_id && f.view.id == c.id) {
            Some(i) => i,
            None => {
                self.shown.push(PluginFloat {
                    tab_id: format!("float:{plugin_id}/{}", c.id),
                    view: PluginView::new(plugin_id, plugin_name, &c.id, plugin_name, Vec::new()),
                    width: 200.0,
                    height: 64.0,
                    start: FloatCorner::BottomRight,
                });
                self.shown.len() - 1
            }
        };
        self.revision += 1;
        let float = &mut self.shown[at];
        float.apply(c);
        if let Some(corner) = c.corner {
            float.start = corner;
        }
        float
    }

    pub fn update(&mut self, plugin_id: &str, c: &FloatContent) -> bool {
        let Some(f) = self.of_mut(plugin_id, &c.id) else { return false };
        f.apply(c);
        self.revision += 1;
        true
    }

    pub fn hide(&mut self, plugin_id: &str, id: &str) {
        let before = self.shown.len();
        self.shown.retain(|f| !(f.view.plugin_id == plugin_id && f.view.id == id));
        if self.shown.len() != before {
            self.revision += 1;
        }
    }

    pub fn drop_plugin(&mut self, plugin_id: &str) {
        let before = self.shown.len();
        self.shown.retain(|f| f.view.plugin_id != plugin_id);
        if self.shown.len() != before {
            self.revision += 1;
        }
    }

    pub fn spot_of(&self, key: &str) -> Option<FloatSpot> {
        self.spots.iter().find(|(k, _)| k == key).map(|(_, s)| *s)
    }

    pub fn move_to(&mut self, key: &str, spot: FloatSpot) {
        match self.spots.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = spot,
            None => self.spots.push((key.to_owned(), spot)),
        }
        self.revision += 1;
    }

    pub fn clear_spots(&mut self) {
        self.spots.clear();
    }

    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        for (key, s) in &self.spots {
            let mut m = Map::new();
            m.insert("corner".into(), Value::from(s.corner.name()));
            m.insert("dx".into(), mx_core::json::f64_value(s.dx.round()));
            m.insert("dy".into(), mx_core::json::f64_value(s.dy.round()));
            out.insert(key.clone(), Value::Object(m));
        }
        Value::Object(out)
    }

    pub fn read_json(&mut self, raw: &Value) {
        self.spots.clear();
        let Value::Object(all) = raw else { return };
        for (key, v) in all {
            let Value::Object(m) = v else { continue };
            let corner = m.get("corner").and_then(FloatCorner::parse);
            let dx = m.get("dx").and_then(Value::as_f64);
            let dy = m.get("dy").and_then(Value::as_f64);
            if let (Some(corner), Some(dx), Some(dy)) = (corner, dx, dy) {
                self.spots.push((key.clone(), FloatSpot { corner, dx, dy }));
            }
        }
    }
}
