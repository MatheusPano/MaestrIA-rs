use gpui::{Pixels, Point, WrappedLine, point, px};

pub struct Row {
    pub start: usize,
    pub y: Pixels,
    pub height: Pixels,
    pub line: WrappedLine,
}

pub struct EditorLayout {
    pub rows: Vec<Row>,
    pub line_height: Pixels,
    pub height: Pixels,
    pub width: Pixels,
    pub wrap_width: Pixels,
    pub gutter: Pixels,
    pub font_size: Pixels,
    pub version: u64,
}

impl EditorLayout {
    pub fn row_of(&self, at: usize) -> usize {
        self.rows.partition_point(|r| r.start <= at).saturating_sub(1)
    }

    pub fn position(&self, at: usize) -> Point<Pixels> {
        let Some(row) = self.rows.get(self.row_of(at)) else {
            return point(px(0.), px(0.));
        };
        let local = (at - row.start).min(row.line.len());
        let p = row
            .line
            .position_for_index(local, self.line_height)
            .unwrap_or_else(|| point(px(0.), px(0.)));
        point(p.x, p.y + row.y)
    }

    pub fn offset_at(&self, p: Point<Pixels>) -> usize {
        let Some(last) = self.rows.last() else {
            return 0;
        };
        let i = self.rows.partition_point(|r| r.y <= p.y).saturating_sub(1);
        let row = &self.rows[i];
        if p.y >= last.y + last.height {
            return last.start + last.line.len();
        }
        let y = (p.y - row.y).clamp(px(0.), row.height - px(1.));
        let local = match row.line.closest_index_for_position(point(p.x.max(px(0.)), y), self.line_height) {
            Ok(i) | Err(i) => i,
        };
        row.start + local.min(row.line.len())
    }
}
