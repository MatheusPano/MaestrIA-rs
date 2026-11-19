#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    Row,
    Column,
}

impl Axis {
    pub fn name(self) -> &'static str {
        match self {
            Axis::Row => "row",
            Axis::Column => "column",
        }
    }

    pub fn min_extent(self) -> f64 {
        match self {
            Axis::Row => MIN_WIDTH,
            Axis::Column => MIN_HEIGHT,
        }
    }
}

pub const MIN_WIDTH: f64 = 240.0;
pub const MIN_HEIGHT: f64 = 140.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DropSide {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

impl DropSide {
    pub fn axis(self) -> Axis {
        match self {
            DropSide::Left | DropSide::Right => Axis::Row,
            _ => Axis::Column,
        }
    }

    pub fn leading(self) -> bool {
        matches!(self, DropSide::Left | DropSide::Top)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaneNode {
    Leaf(String),
    Split(PaneSplit),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaneSplit {
    pub axis: Axis,
    pub children: Vec<PaneNode>,
    pub weights: Vec<f64>,
}

enum Removal {
    Untouched,
    Gone,
    Replaced(PaneNode),
}

impl PaneNode {
    pub fn leaf(id: impl Into<String>) -> PaneNode {
        PaneNode::Leaf(id.into())
    }

    pub fn split(axis: Axis, children: Vec<PaneNode>, weights: Vec<f64>) -> PaneNode {
        PaneNode::Split(PaneSplit {
            axis,
            children,
            weights,
        })
    }

    fn pair(side: DropSide, fresh: PaneNode, other: PaneNode) -> PaneNode {
        let children = if side.leading() {
            vec![fresh, other]
        } else {
            vec![other, fresh]
        };
        PaneNode::split(side.axis(), children, vec![0.5, 0.5])
    }

    pub fn as_split(&self) -> Option<&PaneSplit> {
        match self {
            PaneNode::Split(s) => Some(s),
            PaneNode::Leaf(_) => None,
        }
    }

    pub fn as_split_mut(&mut self) -> Option<&mut PaneSplit> {
        match self {
            PaneNode::Split(s) => Some(s),
            PaneNode::Leaf(_) => None,
        }
    }

    pub fn split_at(&self, path: &[usize]) -> Option<&PaneSplit> {
        let mut node = self;
        for &i in path {
            node = node.as_split()?.children.get(i)?;
        }
        node.as_split()
    }

    pub fn split_at_mut(&mut self, path: &[usize]) -> Option<&mut PaneSplit> {
        let mut node = self;
        for &i in path {
            node = node.as_split_mut()?.children.get_mut(i)?;
        }
        node.as_split_mut()
    }

    pub fn order(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(&mut out);
        out
    }

    fn walk(&self, out: &mut Vec<String>) {
        match self {
            PaneNode::Leaf(id) => out.push(id.clone()),
            PaneNode::Split(s) => s.children.iter().for_each(|c| c.walk(out)),
        }
    }

    pub fn has(&self, tab_id: &str) -> bool {
        self.find_leaf(tab_id).is_some()
    }

    pub fn count(&self) -> usize {
        self.order().len()
    }

    fn find_leaf(&self, tab_id: &str) -> Option<&String> {
        match self {
            PaneNode::Leaf(id) => (id == tab_id).then_some(id),
            PaneNode::Split(s) => s.children.iter().find_map(|c| c.find_leaf(tab_id)),
        }
    }

    fn leaf_mut(&mut self, tab_id: &str) -> Option<&mut String> {
        match self {
            PaneNode::Leaf(id) => (id == tab_id).then_some(id),
            PaneNode::Split(s) => s.children.iter_mut().find_map(|c| c.leaf_mut(tab_id)),
        }
    }

    pub fn swap(&mut self, target_id: &str, tab_id: &str) {
        if let Some(id) = self.leaf_mut(target_id) {
            *id = tab_id.to_owned();
        }
    }

    pub fn swap_leaves(&mut self, a: &str, b: &str) -> bool {
        if a == b || !self.has(a) || !self.has(b) {
            return false;
        }
        self.relabel(a, b);
        true
    }

    fn relabel(&mut self, a: &str, b: &str) {
        match self {
            PaneNode::Leaf(id) if id == a => *id = b.to_owned(),
            PaneNode::Leaf(id) if id == b => *id = a.to_owned(),
            PaneNode::Leaf(_) => {}
            PaneNode::Split(s) => s.children.iter_mut().for_each(|c| c.relabel(a, b)),
        }
    }

    pub fn equalize(&mut self) {
        if let PaneNode::Split(s) = self {
            s.children.iter_mut().for_each(PaneNode::equalize);
            s.equalize();
        }
    }

    pub fn equalize_into(&mut self, outer: Axis) {
        if let Some(turned) = self.transposed(outer) {
            *self = turned;
        }
        if let PaneNode::Split(s) = self {
            s.children.iter_mut().for_each(|c| c.equalize_into(outer));
            s.equalize();
        }
    }

    pub fn equalize_auto(&mut self) {
        let mut best = self.clone();
        best.equalize();
        for outer in [Axis::Column, Axis::Row] {
            let mut turned = self.clone();
            turned.equalize_into(outer);
            if turned.smallest_share() > best.smallest_share() + 1e-9 {
                best = turned;
            }
        }
        *self = best;
    }

    fn smallest_share(&self) -> f64 {
        match self {
            PaneNode::Leaf(_) => 1.0,
            PaneNode::Split(s) => s
                .children
                .iter()
                .zip(&s.weights)
                .map(|(c, w)| w * c.smallest_share())
                .fold(f64::INFINITY, f64::min),
        }
    }

    fn transposed(&self, outer: Axis) -> Option<PaneNode> {
        let s = self.as_split().filter(|s| s.axis != outer)?;
        let lines: Vec<&PaneSplit> =
            s.children.iter().filter_map(PaneNode::as_split).filter(|c| c.axis == outer).collect();
        let depth = lines.first()?.children.len();
        if lines.len() != s.children.len() || lines.iter().any(|c| c.children.len() != depth) {
            return None;
        }
        let turned = (0..depth)
            .map(|i| {
                let cells: Vec<PaneNode> = lines.iter().map(|c| c.children[i].clone()).collect();
                let share = vec![1.0 / cells.len() as f64; cells.len()];
                PaneNode::split(s.axis, cells, share)
            })
            .collect();
        Some(PaneNode::split(outer, turned, vec![1.0 / depth as f64; depth]))
    }

    fn span(&self, axis: Axis) -> f64 {
        match self {
            PaneNode::Split(s) if s.axis == axis => s.children.iter().map(|c| c.span(axis)).sum(),
            _ => 1.0,
        }
    }

    pub fn equalize_at(&mut self, path: &[usize]) -> bool {
        match self.split_at_mut(path) {
            Some(s) => {
                s.equalize();
                true
            }
            None => false,
        }
    }

    pub fn insert(self, tab_id: &str, target_id: &str, side: DropSide) -> PaneNode {
        match self {
            PaneNode::Leaf(ref id) if id != target_id => self,
            PaneNode::Leaf(_) => PaneNode::pair(side, PaneNode::leaf(tab_id), self),
            PaneNode::Split(mut s) => {
                s.insert_at(tab_id, target_id, side);
                PaneNode::Split(s)
            }
        }
    }

    pub fn remove(self, tab_id: &str) -> Option<PaneNode> {
        let mut root = self;
        match root.remove_in(tab_id) {
            Removal::Gone => None,
            Removal::Replaced(n) => Some(n),
            Removal::Untouched => Some(root),
        }
    }

    fn remove_in(&mut self, tab_id: &str) -> Removal {
        let s = match self {
            PaneNode::Leaf(id) => {
                return if id == tab_id {
                    Removal::Gone
                } else {
                    Removal::Untouched
                };
            }
            PaneNode::Split(s) => s,
        };
        for i in 0..s.children.len() {
            match s.children[i].remove_in(tab_id) {
                Removal::Untouched => continue,
                Removal::Gone => {
                    s.children.remove(i);
                    let freed = s.weights.remove(i);
                    spread(&mut s.weights, freed);
                }
                Removal::Replaced(n) => {
                    s.children[i] = n;
                    s.absorb(i);
                }
            }
            break;
        }
        if s.children.len() == 1 {
            return Removal::Replaced(s.children.remove(0));
        }
        Removal::Untouched
    }

    pub fn edge(self, tab_id: &str, side: DropSide) -> PaneNode {
        match self {
            PaneNode::Split(mut s) if s.axis == side.axis() => {
                let n = s.children.len();
                let scale = n as f64 / (n as f64 + 1.0);
                s.weights.iter_mut().for_each(|w| *w *= scale);
                let at = if side.leading() { 0 } else { n };
                s.children.insert(at, PaneNode::leaf(tab_id));
                s.weights.insert(at, 1.0 / (n as f64 + 1.0));
                PaneNode::Split(s)
            }
            other => PaneNode::pair(side, PaneNode::leaf(tab_id), other),
        }
    }

    pub fn same_shape(&self, other: &PaneNode) -> bool {
        match (self, other) {
            (PaneNode::Leaf(a), PaneNode::Leaf(b)) => a == b,
            (PaneNode::Split(a), PaneNode::Split(b)) => {
                a.axis == b.axis
                    && a.children.len() == b.children.len()
                    && a.children
                        .iter()
                        .zip(&b.children)
                        .all(|(x, y)| x.same_shape(y))
            }
            _ => false,
        }
    }
}

impl PaneSplit {
    fn insert_at(&mut self, tab_id: &str, target_id: &str, side: DropSide) -> bool {
        for i in 0..self.children.len() {
            let is_target = match &mut self.children[i] {
                PaneNode::Split(child) => {
                    if child.insert_at(tab_id, target_id, side) {
                        return true;
                    }
                    continue;
                }
                PaneNode::Leaf(id) => id == target_id,
            };
            if !is_target {
                continue;
            }
            if self.axis == side.axis() {
                let at = if side.leading() { i } else { i + 1 };
                let share = self.weights[i] / 2.0;
                self.weights[i] = share;
                self.children.insert(at, PaneNode::leaf(tab_id));
                self.weights.insert(at, share);
            } else {
                let leaf = std::mem::replace(&mut self.children[i], PaneNode::leaf(""));
                self.children[i] = PaneNode::pair(side, PaneNode::leaf(tab_id), leaf);
            }
            return true;
        }
        false
    }

    fn absorb(&mut self, i: usize) {
        let same_axis = matches!(&self.children[i], PaneNode::Split(c) if c.axis == self.axis);
        if !same_axis {
            return;
        }
        let share = self.weights.remove(i);
        let PaneNode::Split(child) = self.children.remove(i) else {
            return;
        };
        for (k, (c, w)) in child.children.into_iter().zip(child.weights).enumerate() {
            self.children.insert(i + k, c);
            self.weights.insert(i + k, share * w);
        }
    }

    pub fn equalize(&mut self) {
        let spans: Vec<f64> = self.children.iter().map(|c| c.span(self.axis)).collect();
        let total: f64 = spans.iter().sum();
        self.weights = spans.into_iter().map(|s| s / total).collect();
    }

    pub fn resize(&mut self, gutter: isize, delta: f64, total: f64) {
        if gutter < 0 || gutter as usize + 1 >= self.children.len() || total <= 0.0 {
            return;
        }
        let g = gutter as usize;
        let floor = (self.axis.min_extent() / total).clamp(0.0, 0.45);
        let (a, b) = (self.weights[g], self.weights[g + 1]);
        let lower = floor - a;
        let upper = b - floor;
        if lower > upper {
            return;
        }
        let moved = (delta / total).clamp(lower, upper);
        if moved == 0.0 {
            return;
        }
        self.weights[g] = a + moved;
        self.weights[g + 1] = b - moved;
    }
}

fn spread(weights: &mut [f64], freed: f64) {
    if weights.is_empty() {
        return;
    }
    let sum: f64 = weights.iter().sum();
    if sum <= 0.0 {
        let n = weights.len() as f64;
        weights.iter_mut().for_each(|w| *w = 1.0 / n);
        return;
    }
    weights.iter_mut().for_each(|w| *w += freed * *w / sum);
}

pub(crate) fn normalize(weights: &mut [f64]) {
    let sum: f64 = weights.iter().sum();
    if sum <= 0.0 {
        let n = weights.len() as f64;
        weights.iter_mut().for_each(|w| *w = 1.0 / n);
        return;
    }
    weights.iter_mut().for_each(|w| *w /= sum);
}

pub fn order(root: Option<&PaneNode>) -> Vec<String> {
    root.map(PaneNode::order).unwrap_or_default()
}

pub fn same_shape(a: Option<&PaneNode>, b: Option<&PaneNode>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same_shape(b),
        _ => false,
    }
}
