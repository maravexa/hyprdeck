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

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another point.
    pub fn distance_to(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Width/height pair in logical pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
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
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// Returns true if the given point falls within this rectangle.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.width && p.y >= self.y && p.y < self.y + self.height
    }

    /// Inset the rect by padding on all sides.
    pub fn inset(&self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            x: self.x + left,
            y: self.y + top,
            width: (self.width - left - right).max(0.0),
            height: (self.height - top - bottom).max(0.0),
        }
    }

    /// Return the center point.
    pub fn center(&self) -> Point {
        Point {
            x: self.x + self.width / 2.0,
            y: self.y + self.height / 2.0,
        }
    }

    /// Split horizontally at x offset from the left edge, returns (left, right).
    pub fn split_h(&self, x: f32) -> (Rect, Rect) {
        let x_clamped = x.clamp(0.0, self.width);
        (
            Rect::new(self.x, self.y, x_clamped, self.height),
            Rect::new(self.x + x_clamped, self.y, self.width - x_clamped, self.height),
        )
    }

    /// Split vertically at y offset from the top edge, returns (top, bottom).
    pub fn split_v(&self, y: f32) -> (Rect, Rect) {
        let y_clamped = y.clamp(0.0, self.height);
        (
            Rect::new(self.x, self.y, self.width, y_clamped),
            Rect::new(self.x, self.y + y_clamped, self.width, self.height - y_clamped),
        )
    }

    /// Convert to `tiny_skia::Rect` (f32-based). Returns `None` if degenerate.
    pub fn to_skia(&self) -> Option<tiny_skia::Rect> {
        tiny_skia::Rect::from_xywh(self.x, self.y, self.width, self.height)
    }

    /// Convert to `tiny_skia::IntRect` (pixel-aligned). Returns `None` if degenerate.
    pub fn to_skia_int(&self) -> Option<tiny_skia::IntRect> {
        tiny_skia::IntRect::from_xywh(
            self.x as i32,
            self.y as i32,
            self.width as u32,
            self.height as u32,
        )
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
