pub const SUBMENU_WIDTH: f32 = 216.;
pub const MARGIN: f32 = 8.;
pub const PAD: f32 = 5.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    #[cfg(test)]
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

pub fn row_offset(heights: impl IntoIterator<Item = f32>, index: usize) -> f32 {
    PAD + heights.into_iter().take(index).sum::<f32>()
}

pub fn submenu_frame(parent: Rect, row_top: f32, content_h: f32, window_w: f32, window_h: f32) -> Rect {
    let mut x = parent.right() - 4.;
    if x + SUBMENU_WIDTH > window_w - MARGIN {
        x = parent.x - SUBMENU_WIDTH + 4.;
    }
    x = x.max(MARGIN);
    let h = content_h.min(window_h - 2. * MARGIN).max(0.);
    let mut y = row_top - PAD;
    if y + h > window_h - MARGIN {
        y = window_h - MARGIN - h;
    }
    y = y.max(MARGIN);
    Rect::new(x, y, SUBMENU_WIDTH, h)
}

pub fn menu_max_height(window_h: f32) -> f32 {
    (window_h - 2. * MARGIN).max(0.)
}
