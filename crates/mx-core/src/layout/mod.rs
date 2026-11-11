pub mod geometry;
pub mod json;
pub mod tree;

pub use geometry::{Area, GripRef, Gutter, Junction, PANE_GAP, PaneRect, auto_split, facing, gutters, junctions, largest, neighbour, rect_of, rects, split_area};
pub use json::{from_json, legacy_two_slot, to_json};
pub use tree::{Axis, DropSide, MIN_HEIGHT, MIN_WIDTH, PaneNode, PaneSplit, order, same_shape};
