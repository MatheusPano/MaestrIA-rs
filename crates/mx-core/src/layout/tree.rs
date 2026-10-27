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
