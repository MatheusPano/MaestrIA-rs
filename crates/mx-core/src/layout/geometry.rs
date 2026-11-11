use crate::layout::tree::{Axis, DropSide, MIN_HEIGHT, MIN_WIDTH, PaneNode, PaneSplit};

pub const PANE_GAP: f64 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub struct PaneRect {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl PaneRect {
    pub fn area(&self) -> f64 {
        self.w * self.h
    }

    pub fn longer_axis(&self) -> Axis {
        if self.w >= self.h { Axis::Row } else { Axis::Column }
    }

    pub fn splits(&self, axis: Axis, gap: f64) -> bool {
        let (extent, min) = match axis {
            Axis::Row => (self.w, MIN_WIDTH),
            Axis::Column => (self.h, MIN_HEIGHT),
        };
        (extent - gap) / 2.0 >= min
    }
}

pub fn trailing(axis: Axis) -> DropSide {
    match axis {
        Axis::Row => DropSide::Right,
        Axis::Column => DropSide::Bottom,
    }
}

fn other(axis: Axis) -> Axis {
    match axis {
        Axis::Row => Axis::Column,
        Axis::Column => Axis::Row,
    }
}

pub fn rects(tree: &PaneNode, w: f64, h: f64, gap: f64) -> Vec<PaneRect> {
    let mut out = Vec::new();
    lay(tree, Area { x: 0.0, y: 0.0, w: w.max(0.0), h: h.max(0.0) }, gap, &mut out);
    out
}

fn lay(node: &PaneNode, at: Area, gap: f64, out: &mut Vec<PaneRect>) {
    match node {
        PaneNode::Leaf(id) => out.push(PaneRect { id: id.clone(), x: at.x, y: at.y, w: at.w, h: at.h }),
        PaneNode::Split(s) => lay_children(s, at, gap, |i, a| lay(&s.children[i], a, gap, out)),
    }
}

pub fn rect_of(tree: &PaneNode, id: &str, w: f64, h: f64, gap: f64) -> Option<PaneRect> {
    rects(tree, w, h, gap).into_iter().find(|r| r.id == id)
}

fn split_of(r: &PaneRect, gap: f64) -> Option<DropSide> {
    let first = r.longer_axis();
    [first, other(first)].into_iter().find(|&a| r.splits(a, gap)).map(trailing)
}

pub fn auto_split(tree: &PaneNode, target: &str, w: f64, h: f64, gap: f64) -> Option<(String, DropSide)> {
    let all = rects(tree, w, h, gap);
    if let Some(side) = all.iter().find(|r| r.id == target).and_then(|r| split_of(r, gap)) {
        return Some((target.to_owned(), side));
    }
    let largest = biggest(all.iter().filter(|r| r.id != target))?;
    split_of(largest, gap).map(|side| (largest.id.clone(), side))
}

pub fn facing(r: &PaneRect, side: DropSide, at: (f64, f64)) -> DropSide {
    let (x, y) = at;
    let inside = x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h;
    if !inside {
        return side;
    }
    match side.axis() {
        Axis::Row if x < r.x + r.w / 2.0 => DropSide::Left,
        Axis::Row => DropSide::Right,
        Axis::Column if y < r.y + r.h / 2.0 => DropSide::Top,
        Axis::Column => DropSide::Bottom,
    }
}

fn biggest<'a>(rects: impl Iterator<Item = &'a PaneRect>) -> Option<&'a PaneRect> {
    rects.reduce(|best, r| if r.area() > best.area() { r } else { best })
}

pub fn largest(tree: &PaneNode, w: f64, h: f64, gap: f64) -> Option<PaneRect> {
    biggest(rects(tree, w, h, gap).iter()).cloned()
}

pub fn neighbour<'a>(rects: &'a [PaneRect], from: &str, side: DropSide) -> Option<&'a PaneRect> {
    let r = rects.iter().find(|r| r.id == from)?;
    let span = |p: &PaneRect| match side.axis() {
        Axis::Row => (p.y, p.y + p.h),
        Axis::Column => (p.x, p.x + p.w),
    };
    let distance = |c: &PaneRect| match side {
        DropSide::Right => c.x - (r.x + r.w),
        DropSide::Left => r.x - (c.x + c.w),
        DropSide::Bottom => c.y - (r.y + r.h),
        DropSide::Top => r.y - (c.y + c.h),
        DropSide::Center => f64::NAN,
    };
    let (lo, hi) = span(r);
    let overlap = |c: &PaneRect| {
        let (a, b) = span(c);
        b.min(hi) - a.max(lo)
    };
    rects
        .iter()
        .filter(|c| c.id != from && distance(c) >= -0.5 && overlap(c) > 0.5)
        .min_by(|a, b| {
            let key = |c: &PaneRect| (distance(c), -overlap(c), span(c).0);
            key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal)
        })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Area {
    fn touches(&self, o: &Area) -> bool {
        const EPS: f64 = 0.5;
        self.x <= o.x + o.w + EPS && o.x <= self.x + self.w + EPS && self.y <= o.y + o.h + EPS && o.y <= self.y + self.h + EPS
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GripRef {
    pub path: Vec<usize>,
    pub gutter: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gutter {
    pub grip: GripRef,
    pub axis: Axis,
    pub strip: Area,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Junction {
    pub x: f64,
    pub y: f64,
    pub rows: Vec<GripRef>,
    pub columns: Vec<GripRef>,
}

fn walk(node: &PaneNode, path: &mut Vec<usize>, at: Area, gap: f64, visit: &mut dyn FnMut(&[usize], &PaneSplit, Area)) {
    let PaneNode::Split(s) = node else { return };
    visit(path, s, at);
    let mut kids = Vec::new();
    lay_children(s, at, gap, |i, a| kids.push((i, a)));
    for (i, a) in kids {
        path.push(i);
        walk(&s.children[i], path, a, gap, visit);
        path.pop();
    }
}

fn lay_children(s: &PaneSplit, at: Area, gap: f64, mut each: impl FnMut(usize, Area)) {
    let n = s.children.len();
    if n == 0 {
        return;
    }
    let total = match s.axis {
        Axis::Row => at.w,
        Axis::Column => at.h,
    };
    let free = (total - gap * (n - 1) as f64).max(0.0);
    let sum: f64 = s.weights.iter().take(n).sum();
    let mut off = 0.0;
    for i in 0..n {
        let share = match s.weights.get(i) {
            Some(wt) if sum > 0.0 => wt / sum,
            _ => 1.0 / n as f64,
        };
        let size = free * share;
        let a = match s.axis {
            Axis::Row => Area { x: at.x + off, w: size, ..at },
            Axis::Column => Area { y: at.y + off, h: size, ..at },
        };
        each(i, a);
        off += size + gap;
    }
}

pub fn split_area(tree: &PaneNode, path: &[usize], w: f64, h: f64, gap: f64) -> Option<Area> {
    let mut found = None;
    let whole = Area { x: 0.0, y: 0.0, w: w.max(0.0), h: h.max(0.0) };
    walk(tree, &mut Vec::new(), whole, gap, &mut |p, _, a| {
        if p == path {
            found = Some(a);
        }
    });
    found
}

pub fn gutters(tree: &PaneNode, w: f64, h: f64, gap: f64) -> Vec<Gutter> {
    let mut out = Vec::new();
    let whole = Area { x: 0.0, y: 0.0, w: w.max(0.0), h: h.max(0.0) };
    walk(tree, &mut Vec::new(), whole, gap, &mut |p, s, at| {
        lay_children(s, at, gap, |i, a| {
            if i + 1 == s.children.len() {
                return;
            }
            let strip = match s.axis {
                Axis::Row => Area { x: a.x + a.w, w: gap, ..a },
                Axis::Column => Area { y: a.y + a.h, h: gap, ..a },
            };
            out.push(Gutter { grip: GripRef { path: p.to_vec(), gutter: i }, axis: s.axis, strip });
        });
    });
    out
}

pub fn junctions(tree: &PaneNode, w: f64, h: f64, gap: f64) -> Vec<Junction> {
    let all = gutters(tree, w, h, gap);
    let mut out: Vec<Junction> = Vec::new();
    for v in all.iter().filter(|g| g.axis == Axis::Row) {
        for c in all.iter().filter(|g| g.axis == Axis::Column && g.strip.touches(&v.strip)) {
            let (x, y) = (v.strip.x + gap / 2.0, c.strip.y + gap / 2.0);
            let near = out.iter_mut().find(|j| (j.x - x).abs() < gap && (j.y - y).abs() < gap);
            let j = match near {
                Some(j) => j,
                None => {
                    out.push(Junction { x, y, rows: Vec::new(), columns: Vec::new() });
                    out.last_mut().expect("just pushed")
                }
            };
            if !j.rows.contains(&v.grip) {
                j.rows.push(v.grip.clone());
            }
            if !j.columns.contains(&c.grip) {
                j.columns.push(c.grip.clone());
            }
        }
    }
    out
}
