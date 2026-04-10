use serde::{Deserialize, Serialize};

/// Screen edge for panel anchoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

/// A 2-D point in logical pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Width/height pair in logical pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Axis-aligned bounding rectangle in logical pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Returns true if the given point falls within this rectangle.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.width && p.y >= self.y && p.y < self.y + self.height
    }
}

/// Display geometry abstraction.
///
/// For standard rectangular displays, `usable_region` and `edge_path` are `None`.
/// Non-`None` variants are reserved for future circular / curved display support.
#[derive(Debug)]
pub struct DisplayGeometry {
    /// Full display bounds.
    pub bounds: Rect,
    /// Optional polygon defining the usable region (non-rectangular displays).
    pub usable_region: Option<Vec<Point>>,
    /// Optional path along the display edge that a bar should follow.
    pub edge_path: Option<Vec<Point>>,
}
