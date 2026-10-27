use gpui::{App, Bounds, Hsla, PathBuilder, Pixels, SharedString, TextRun, Window, font, point, px};

use super::fold::FOLD_WIDTH;
use super::layout::EditorLayout;
use super::style::Gutter;
use crate::chrome::style::MONO;

const NUMBER_PAD: f32 = 12.;

pub fn digit_width(font_size: Pixels, window: &Window) -> Pixels {
    let ts = window.text_system();
    let id = ts.resolve_font(&font(MONO));
    ts.advance(id, font_size, '0').map_or(font_size * 0.6, |s| s.width)
}

pub fn widths(g: &Gutter, lines: usize, window: &Window) -> (Pixels, Pixels) {
    let digits = lines.to_string().len().max(2);
    let numbers = (digit_width(px(g.font_size), window) * digits as f32 + px(NUMBER_PAD)).round();
    let fold = if g.fold.is_some() { px(FOLD_WIDTH) } else { px(0.) };
    (numbers, numbers + fold)
}

fn run(len: usize, color: Hsla) -> TextRun {
    TextRun { len, font: font(MONO), color, background_color: None, underline: None, strikethrough: None }
}

fn triangle(center: gpui::Point<Pixels>, size: Pixels, folded: bool, color: Hsla, window: &mut Window) {
    let h = size / 2.;
    let mut b = PathBuilder::fill();
    if folded {
        b.move_to(point(center.x - h * 0.5, center.y - h));
        b.line_to(point(center.x + h * 0.6, center.y));
        b.line_to(point(center.x - h * 0.5, center.y + h));
    } else {
        b.move_to(point(center.x - h, center.y - h * 0.5));
        b.line_to(point(center.x + h, center.y - h * 0.5));
        b.line_to(point(center.x, center.y + h * 0.6));
    }
    b.close();
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

pub fn paint(layout: &EditorLayout, g: &Gutter, bounds: Bounds<Pixels>, origin_y: Pixels, current: usize, window: &mut Window, cx: &mut App) {
    let lh = layout.line_height;
    let top = bounds.origin.y - origin_y - lh;
    let bottom = top + bounds.size.height + lh * 2.;
    let first = layout.rows.partition_point(|r| r.y + r.height < top);
    let size = px(g.font_size);
    for row in layout.rows.iter().skip(first) {
        if row.y > bottom {
            break;
        }
        let color = if row.index == current { g.current } else { g.color };
        let label = SharedString::from((row.index + 1).to_string());
        let shaped = window.text_system().shape_line(label.clone(), size, &[run(label.len(), color)], None);
        let x = bounds.origin.x + layout.numbers - px(NUMBER_PAD / 2.) - shaped.width;
        shaped.paint(point(x, origin_y + row.y), lh, window, cx).ok();
        let Some(fold) = g.fold else { continue };
        if layout.chunks.iter().any(|(s, _)| *s == row.index) {
            let folded = layout.folded.iter().any(|(s, _)| *s == row.index);
            let center = point(bounds.origin.x + layout.numbers + px(FOLD_WIDTH / 2.), origin_y + row.y + lh / 2.);
            triangle(center, (layout.font_size * 0.55).round(), folded, fold, window);
        }
    }
}
