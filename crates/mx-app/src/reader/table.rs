use gpui::{
    AnyElement, Div, FontWeight, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, TextRun, div, prelude::*, px, relative,
};
use mx_claude::markdown::{MdAlign, MdKind, MdNode};

use super::blocks::Painter;
use super::inline::Inline;
use super::style::Marks;

const PAD_X: f32 = 10.;
const PAD_Y: f32 = 6.;

struct Cell {
    inline: Inline,
    min: f32,
    max: f32,
}

pub fn intrinsic_widths(min: &[f32], max: &[f32], avail: f32) -> Vec<f32> {
    let sum_max: f32 = max.iter().sum();
    let sum_min: f32 = min.iter().sum();
    if sum_max <= avail {
        return max.to_vec();
    }
    if sum_min >= avail || sum_max <= sum_min {
        return min.to_vec();
    }
    let share = (avail - sum_min) / (sum_max - sum_min);
    min.iter()
        .zip(max)
        .map(|(lo, hi)| lo + (hi - lo) * share)
        .collect()
}

impl Painter<'_> {
    fn measure(&self, text: &str, runs: &[TextRun], size: f32) -> f32 {
        let line = text.replace('\n', " ");
        let shaped = self
            .text_system
            .shape_line(SharedString::from(line), px(size), runs, None);
        f32::from(shaped.width).ceil()
    }

    fn cell(&self, node: &MdNode, header: bool, size: f32) -> Cell {
        let marks = Marks {
            bold: header,
            ..Marks::default()
        };
        let inline = Inline::collect(&node.children, self.sheet.fg, marks, self.sheet);
        let max = self.measure(&inline.text, inline.runs(), size);
        let min = words(&inline.text)
            .filter_map(|(at, word)| {
                let mut run = inline.run_at(at)?.clone();
                run.len = word.len();
                Some(self.measure(word, &[run], size))
            })
            .fold(0., f32::max);
        Cell { inline, min, max }
    }

    pub fn table(&self, node: &MdNode) -> AnyElement {
        let sheet = self.sheet;
        let size = sheet.table_size();
        let alignments = match &node.kind {
            MdKind::Table { alignments } => alignments.clone(),
            _ => Vec::new(),
        };
        let rows: Vec<(bool, Vec<Cell>)> = node
            .children
            .iter()
            .map(|row| {
                let header = matches!(row.kind, MdKind::TableRow { header: true });
                let cells = row.children.iter().map(|c| self.cell(c, header, size)).collect();
                (header, cells)
            })
            .collect();
        let cols = rows.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
        let chrome = PAD_X * 2. + 1.;
        let mut min = vec![chrome; cols];
        let mut max = vec![chrome; cols];
        for (_, cells) in &rows {
            for (i, cell) in cells.iter().enumerate() {
                min[i] = min[i].max(cell.min + chrome);
                max[i] = max[i].max(cell.max + chrome);
            }
        }
        let avail = f32::from(self.width).max(120.) - 2.;
        let widths = intrinsic_widths(&min, &max, avail);
        let total: f32 = widths.iter().sum::<f32>() + 1.;
        let last_row = rows.len().saturating_sub(1);
        let body = rows.into_iter().enumerate().map(|(r, (header, cells))| {
            let mut row = div().flex().flex_row();
            if header {
                row = row.bg(sheet.bg_sidebar);
            }
            let mut cells = cells.into_iter();
            for (c, width) in widths.iter().enumerate() {
                let align = alignments.get(c).copied().unwrap_or(MdAlign::None);
                let content = cells.next().map(|cell| cell.inline);
                row = row.child(self.table_cell(content, *width, align, header, c + 1 < cols, r < last_row));
            }
            row
        });
        let grid = div()
            .flex_none()
            .w(px(total))
            .border_1()
            .border_color(sheet.border)
            .flex()
            .flex_col()
            .children(body);
        div()
            .id(self.id("table"))
            .overflow_x_scroll()
            .child(grid)
            .into_any_element()
    }

    fn table_cell(
        &self,
        content: Option<Inline>,
        width: f32,
        align: MdAlign,
        header: bool,
        right: bool,
        bottom: bool,
    ) -> Div {
        let sheet = self.sheet;
        let mut cell = div()
            .flex_none()
            .w(px(width))
            .px(px(PAD_X))
            .py(px(PAD_Y))
            .text_size(px(sheet.table_size()))
            .line_height(relative(1.4))
            .border_color(sheet.border);
        if header {
            cell = cell.font_weight(FontWeight::SEMIBOLD);
        }
        if right {
            cell = cell.border_r_1();
        }
        if bottom {
            cell = cell.border_b_1();
        }
        cell = match align {
            MdAlign::Center => cell.text_center(),
            MdAlign::Right => cell.text_right(),
            _ => cell,
        };
        match content {
            Some(inline) => cell.child(inline.into_element(self.id("td"), self.follow.as_ref())),
            None => cell,
        }
    }
}

fn words(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split(char::is_whitespace)
        .filter(|w| !w.is_empty())
        .map(move |w| (w.as_ptr() as usize - text.as_ptr() as usize, w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_keep_their_natural_width_when_the_table_fits() {
        assert_eq!(intrinsic_widths(&[10., 20.], &[50., 60.], 200.), vec![50., 60.]);
    }

    #[test]
    fn columns_shrink_toward_their_longest_word_when_the_table_is_too_wide() {
        assert_eq!(intrinsic_widths(&[10., 20.], &[110., 20.], 80.), vec![60., 20.]);
    }

    #[test]
    fn columns_stop_at_their_longest_word_and_the_table_scrolls() {
        assert_eq!(intrinsic_widths(&[50., 60.], &[100., 100.], 80.), vec![50., 60.]);
    }

    #[test]
    fn words_report_their_byte_offsets() {
        let found: Vec<_> = words("ação  em\ttabela").collect();
        assert_eq!(found, vec![(0, "ação"), (8, "em"), (11, "tabela")]);
    }
}
