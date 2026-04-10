pub mod dock;
pub mod horizontal;
pub mod vertical;

pub use dock::{DockLayout, DockMagnification};
pub use horizontal::HorizontalLayout;
pub use vertical::VerticalLayout;

use crate::geometry::{Rect, Size};

/// Discriminated union of the three supported layout strategies.
#[derive(Debug)]
pub enum LayoutEngine {
    Horizontal(HorizontalLayout),
    Vertical(VerticalLayout),
    Dock(DockLayout),
}

/// Output of a layout pass: per-module bounding boxes and the total consumed size.
#[derive(Debug)]
pub struct LayoutResult {
    /// Map from module ID to its assigned bounding box within the panel.
    pub module_bounds: Vec<(String, Rect)>,
    /// Total size required by all modules (may equal the panel size or be smaller).
    pub total_size: Size,
}

/// Module grouping for three-section layouts (horizontal or vertical).
///
/// Each field holds an ordered list of module IDs.  The center group is always
/// geometrically centered; start and end groups are flush to their respective edges.
#[derive(Debug, Default)]
pub struct ModuleGroups {
    pub start: Vec<String>,
    pub center: Vec<String>,
    pub end: Vec<String>,
}
